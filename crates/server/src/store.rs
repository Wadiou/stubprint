//! What the server keeps on disk: one SQLite database with the address of the API, the printers
//! with their status, the receipts and the connections. It is read when the server starts. After
//! that the server works from memory, and every change is also written to the database, on a thread
//! of its own so that writing never slows a printer down. The bytes a printer holds while it cannot
//! print are not kept.

use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::thread::JoinHandle;

use rusqlite::{params, Connection as Db, OptionalExtension};
use stubprint_core::{Dialect, PrinterStatus, Profile};

use crate::config::PrinterConfig;
use crate::hub::Connection;
use crate::migrations;
use crate::printer::{Usage, VirtualPrinter};
use crate::settings::{Settings, SettingsChange};

/// One printer as it is saved: its settings, what it reports, and its paper.
#[derive(Debug)]
pub(crate) struct SavedPrinter {
    name: String,
    listen: String,
    dialect: String,
    profile: String,
    settings: Settings,
    status: PrinterStatus,
    paper_left: f64,
    usage: Usage,
}

impl SavedPrinter {
    pub(crate) fn of(printer: &VirtualPrinter) -> Self {
        Self {
            name: printer.name.clone(),
            listen: printer.listen.to_string(),
            dialect: printer.dialect.name().to_string(),
            profile: printer.profile().name.clone(),
            settings: printer.settings(),
            status: printer.status(),
            paper_left: printer.paper_left(),
            usage: printer.usage(),
        }
    }

    /// How much paper was left on its roll.
    pub(crate) fn paper_left(&self) -> f64 {
        self.paper_left
    }

    /// What it had done.
    pub(crate) fn usage(&self) -> Usage {
        self.usage
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn status(&self) -> PrinterStatus {
        self.status
    }

    /// The settings to start the printer with. A profile name that no longer exists becomes the
    /// generic profile.
    pub(crate) fn config(&self) -> Result<PrinterConfig, String> {
        Ok(PrinterConfig {
            name: self.name.clone(),
            listen: self
                .listen
                .parse()
                .map_err(|_| format!("`{}` is not an address", self.listen))?,
            dialect: Dialect::from_name(&self.dialect)
                .ok_or_else(|| format!("`{}` is not a dialect", self.dialect))?,
            profile: Profile::named(&self.profile).unwrap_or_else(Profile::generic),
            settings: self.settings,
        })
    }
}

/// One receipt as it is saved: the bytes it was made from, and what read them. The rest of it, its
/// lines and its picture, is made again from these when the server starts.
#[derive(Debug)]
pub(crate) struct SavedReceipt {
    pub(crate) id: u64,
    pub(crate) printer: String,
    pub(crate) dialect: String,
    pub(crate) profile: String,
    pub(crate) at: u64,
    pub(crate) state: String,
    pub(crate) raw: Vec<u8>,
}

/// Everything the database had when it was opened.
#[derive(Debug, Default)]
pub(crate) struct Loaded {
    pub(crate) http: Option<SocketAddr>,
    pub(crate) printers: Vec<SavedPrinter>,
    pub(crate) receipts: Vec<SavedReceipt>,
    pub(crate) connections: Vec<Connection>,
    /// The id the next receipt gets. Numbers are not used again, even for receipts that were
    /// removed.
    pub(crate) next_receipt: u64,
    /// The id the next connection gets.
    pub(crate) next_connection: u64,
}

/// A change to write.
#[derive(Debug)]
pub(crate) enum Command {
    Http(SocketAddr),
    /// All the printers, which replace the ones that were saved.
    Printers(Vec<SavedPrinter>),
    Receipt(SavedReceipt),
    RemoveReceipt(u64),
    /// Removes the receipts of one printer, or of all.
    ClearReceipts(Option<String>),
    /// A connection, as it is when it closes.
    Connection(Connection),
}

/// The database, and the thread that writes to it.
#[derive(Debug)]
pub(crate) struct Store {
    sender: Mutex<Option<mpsc::Sender<Command>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Store {
    /// Opens the database, which is made when it does not exist, and reads what it has. With
    /// `reset` what was there is deleted first.
    pub(crate) fn open(path: &Path, reset: bool) -> io::Result<(Store, Loaded)> {
        if reset {
            for suffix in ["", "-wal", "-shm"] {
                let mut file = path.as_os_str().to_owned();
                file.push(suffix);
                match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        if let Some(folder) = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
        {
            std::fs::create_dir_all(folder)?;
        }
        let unusable = |error: rusqlite::Error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} cannot be used as the database: {error}", path.display()),
            )
        };
        let db = Db::open(path).map_err(unusable)?;
        prepare(&db).map_err(unusable)?;
        let loaded = load(&db).map_err(unusable)?;

        let (sender, receiver) = mpsc::channel::<Command>();
        let thread = std::thread::Builder::new()
            .name("stubprint-database".into())
            .spawn(move || {
                for command in receiver {
                    if let Err(error) = write(&db, command) {
                        eprintln!("stubprint: cannot write to the database: {error}");
                    }
                }
            })?;
        let store = Store {
            sender: Mutex::new(Some(sender)),
            thread: Mutex::new(Some(thread)),
        };
        Ok((store, loaded))
    }

    /// Hands a change to the thread that writes. It does not wait for it to be written.
    pub(crate) fn send(&self, command: Command) {
        if let Some(sender) = self
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            let _ = sender.send(command);
        }
    }

    /// Waits until everything sent is written, and writes nothing after that.
    pub(crate) fn close(&self) {
        drop(
            self.sender
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take(),
        );
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.close();
    }
}

/// Opens the database for use: applies the changes it has not had, and refuses one that is not
/// ours.
fn prepare(db: &Db) -> rusqlite::Result<()> {
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    db.pragma_update(None, "synchronous", "NORMAL")?;
    migrations::apply(db)
}

fn int(number: u64) -> i64 {
    i64::try_from(number).unwrap_or(i64::MAX)
}

fn count(number: usize) -> u64 {
    u64::try_from(number).unwrap_or(u64::MAX)
}

fn unsigned(number: i64) -> u64 {
    u64::try_from(number).unwrap_or(0)
}

fn load(db: &Db) -> rusqlite::Result<Loaded> {
    let http = db
        .query_row("SELECT value FROM settings WHERE key = 'http'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .and_then(|text| text.parse().ok());

    let mut printers = Vec::new();
    let mut statement = db.prepare(
        "SELECT name, listen, dialect, profile, status, settings, paper_left, receipts, cuts,
                line_feeds, paper_used
         FROM printers ORDER BY position",
    )?;
    let rows = statement.query_map([], |row| {
        let status: String = row.get(4)?;
        let settings: String = row.get(5)?;
        Ok(SavedPrinter {
            name: row.get(0)?,
            listen: row.get(1)?,
            dialect: row.get(2)?,
            profile: row.get(3)?,
            status: serde_json::from_str(&status).unwrap_or_default(),
            // A setting the text does not have is the default, so a setting added later is no change
            // to the database.
            settings: serde_json::from_str::<SettingsChange>(&settings)
                .unwrap_or_default()
                .applied_to(Settings::default()),
            paper_left: row.get(6)?,
            usage: Usage {
                receipts: unsigned(row.get(7)?),
                cuts: unsigned(row.get(8)?),
                line_feeds: unsigned(row.get(9)?),
                paper_used: row.get(10)?,
            },
        })
    })?;
    for printer in rows {
        printers.push(printer?);
    }

    let mut receipts = Vec::new();
    let mut statement = db.prepare(
        "SELECT id, printer, dialect, profile, at, raw, state FROM receipts ORDER BY id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SavedReceipt {
            id: unsigned(row.get(0)?),
            printer: row.get(1)?,
            dialect: row.get(2)?,
            profile: row.get(3)?,
            at: unsigned(row.get(4)?),
            raw: row.get(5)?,
            state: row.get(6)?,
        })
    })?;
    for receipt in rows {
        receipts.push(receipt?);
    }

    let mut connections = Vec::new();
    let mut statement = db.prepare("SELECT data FROM connections ORDER BY id")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for data in rows {
        // A connection that cannot be read is left out, so one bad row does not stop the server.
        if let Ok(connection) = serde_json::from_str::<Connection>(&data?) {
            connections.push(connection);
        }
    }

    Ok(Loaded {
        http,
        printers,
        next_receipt: next_id(db, "receipts")?,
        next_connection: next_id(db, "connections")?,
        receipts,
        connections,
    })
}

/// The id after the highest a table ever gave, which `AUTOINCREMENT` keeps even when the rows are
/// deleted.
fn next_id(db: &Db, table: &str) -> rusqlite::Result<u64> {
    let last: Option<i64> = db
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()?;
    Ok(unsigned(last.unwrap_or(0)) + 1)
}

fn write(db: &Db, command: Command) -> rusqlite::Result<()> {
    match command {
        Command::Http(address) => {
            db.execute(
                "INSERT INTO settings (key, value) VALUES ('http', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [address.to_string()],
            )?;
        }
        Command::Printers(printers) => {
            let transaction = db.unchecked_transaction()?;
            transaction.execute("DELETE FROM printers", [])?;
            for (position, printer) in printers.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO printers
                     (position, name, listen, dialect, profile, status, settings, paper_left,
                      receipts, cuts, line_feeds, paper_used)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        int(count(position)),
                        printer.name,
                        printer.listen,
                        printer.dialect,
                        printer.profile,
                        serde_json::to_string(&printer.status).unwrap_or_else(|_| "{}".to_string()),
                        serde_json::to_string(&printer.settings)
                            .unwrap_or_else(|_| "{}".to_string()),
                        printer.paper_left,
                        int(printer.usage.receipts),
                        int(printer.usage.cuts),
                        int(printer.usage.line_feeds),
                        printer.usage.paper_used,
                    ],
                )?;
            }
            transaction.commit()?;
        }
        Command::Receipt(receipt) => {
            db.execute(
                "INSERT OR REPLACE INTO receipts (id, printer, dialect, profile, at, raw, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    int(receipt.id),
                    receipt.printer,
                    receipt.dialect,
                    receipt.profile,
                    int(receipt.at),
                    receipt.raw,
                    receipt.state,
                ],
            )?;
        }
        Command::RemoveReceipt(id) => {
            db.execute("DELETE FROM receipts WHERE id = ?1", [int(id)])?;
        }
        Command::ClearReceipts(Some(printer)) => {
            db.execute("DELETE FROM receipts WHERE printer = ?1", [printer])?;
        }
        Command::ClearReceipts(None) => {
            db.execute("DELETE FROM receipts", [])?;
        }
        Command::Connection(connection) => {
            let data = serde_json::to_string(&connection)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            db.execute(
                "INSERT OR REPLACE INTO connections (id, printer, data) VALUES (?1, ?2, ?3)",
                params![int(connection.id), connection.printer, data],
            )?;
        }
    }
    Ok(())
}
