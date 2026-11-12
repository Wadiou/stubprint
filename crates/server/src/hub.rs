use std::collections::HashMap;
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use stubprint_core::{Dialect, Event, Printer, PrinterStatus, Profile, Receipt, Snapshot};
use tokio::sync::broadcast;
use tokio::time::Instant;
use utoipa::ToSchema;

use crate::config::PrinterConfig;
use crate::listener;
use crate::pictures::{PictureKey, Pictures};
use crate::printer::VirtualPrinter;
use crate::store::{Command, Loaded, SavedPrinter, SavedReceipt, Store};
use crate::wire::{PrinterInfo, ReceiptSummary};

/// A receipt the server keeps, and where it came from.
#[derive(Debug)]
pub struct Stored {
    pub id: u64,
    pub printer: Arc<VirtualPrinter>,
    /// The model the printer was when it printed this, which draws it.
    pub profile: Arc<Profile>,
    /// When it was cut, in milliseconds since 1970.
    pub at: u64,
    pub receipt: Receipt,
}

/// One piece of a connection's bytes as it arrived.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Chunk {
    /// Milliseconds after the connection opened.
    pub at: u64,
    /// How many bytes came in this piece.
    pub bytes: usize,
}

/// How many pieces of one connection are told apart. Later ones are added to the last.
const MAX_CHUNKS: usize = 10_000;

/// One program talking to a printer: when, how, and what came of it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Connection {
    /// The number of the connection, which is never used again.
    pub id: u64,
    /// The name of the printer it connected to.
    pub printer: String,
    /// The address of the program, or `http` for a request to print.
    pub peer: String,
    /// Milliseconds since 1970.
    pub opened_at: u64,
    /// When it closed, in milliseconds since 1970, or nothing while it is open.
    pub closed_at: Option<u64>,
    /// How many bytes the program sent.
    pub bytes_in: usize,
    /// How many bytes the printer answered.
    pub bytes_out: usize,
    /// The pieces the bytes came in.
    pub chunks: Vec<Chunk>,
    /// The receipts it printed.
    pub receipts: Vec<u64>,
}

/// What a connection is at the moment it changes: it opened, bytes came in or went out, it printed
/// a receipt, or it closed. The pieces are not repeated, which would make a long job quadratic:
/// only how many there are, and the one that just came in.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConnectionChange {
    /// The number of the connection, which is never used again.
    pub id: u64,
    /// The name of the printer it connected to.
    pub printer: String,
    /// The address of the program, or `http` for a request to print.
    pub peer: String,
    /// False once it closed.
    pub open: bool,
    /// Milliseconds since 1970.
    pub opened_at: u64,
    /// Milliseconds since 1970, or nothing while it is open.
    pub closed_at: Option<u64>,
    /// How many bytes the program has sent so far.
    pub bytes_in: usize,
    /// How many bytes the printer has answered so far.
    pub bytes_out: usize,
    /// How many pieces the bytes have come in so far.
    pub pieces: usize,
    /// The receipts it printed so far.
    pub receipts: Vec<u64>,
    /// The piece that just came in, when this change is one.
    pub chunk: Option<Chunk>,
}

impl ConnectionChange {
    fn of(connection: &Connection, chunk: Option<Chunk>) -> Self {
        Self {
            id: connection.id,
            printer: connection.printer.clone(),
            peer: connection.peer.clone(),
            open: connection.closed_at.is_none(),
            opened_at: connection.opened_at,
            closed_at: connection.closed_at,
            bytes_in: connection.bytes_in,
            bytes_out: connection.bytes_out,
            pieces: connection.chunks.len(),
            receipts: connection.receipts.clone(),
            chunk,
        }
    }
}

/// What happened, for the ones who watch the live events.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Notice {
    Receipt(ReceiptSummary),
    /// Receipts were cleared: all of them, or the ones of a printer.
    Cleared {
        printer: Option<String>,
    },
    /// One receipt was removed.
    ReceiptRemoved {
        id: u64,
    },
    Status {
        printer: String,
        status: PrinterStatus,
    },
    /// A connection opened, got or gave bytes, printed a receipt or closed.
    Connection(ConnectionChange),
    /// A printer was added while the server runs, or at its start.
    PrinterAdded(PrinterInfo),
    /// A printer was set to another model.
    PrinterChanged(PrinterInfo),
    /// A printer was removed, and its receipts were kept.
    PrinterRemoved {
        name: String,
    },
    /// How many bytes a printer holds changed, because it cannot print.
    Held {
        /// The name of the printer.
        printer: String,
        /// How many bytes it holds now.
        held: usize,
    },
    /// A command was read, while the job is still being sent. Only sent to those who ask for the
    /// actions, since a long job makes thousands.
    Action {
        connection: u64,
        printer: String,
        /// Where its bytes start and end in what the connection sent.
        start: usize,
        end: usize,
        #[schema(value_type = Event)]
        event: Arc<Event>,
        #[schema(value_type = Snapshot)]
        state: Arc<Snapshot>,
    },
}

impl Notice {
    /// The name of the event on the wire.
    pub fn name(&self) -> &'static str {
        match self {
            Notice::Receipt(_) => "receipt",
            Notice::Cleared { .. } => "cleared",
            Notice::ReceiptRemoved { .. } => "receipt_removed",
            Notice::Status { .. } => "status",
            Notice::Connection(_) => "connection",
            Notice::Action { .. } => "action",
            Notice::PrinterAdded(_) => "printer_added",
            Notice::PrinterChanged(_) => "printer_changed",
            Notice::PrinterRemoved { .. } => "printer_removed",
            Notice::Held { .. } => "held",
        }
    }
}

#[derive(Debug, Default)]
struct Log {
    connections: VecDeque<Connection>,
    next_id: u64,
}

#[derive(Debug, Default)]
struct Shelf {
    receipts: VecDeque<Arc<Stored>>,
    /// The id the next receipt gets. The first is 1.
    next_id: u64,
    /// The pictures drawn of these receipts.
    pictures: Pictures,
}

/// What every connection and every request shares: the printers, the receipts they printed and
/// the notices about it.
#[derive(Debug)]
pub struct Hub {
    printers: RwLock<Vec<Arc<VirtualPrinter>>>,
    shelf: Mutex<Shelf>,
    log: Mutex<Log>,
    notices: broadcast::Sender<Notice>,
    /// How many watchers want every action, so they are only made when someone looks.
    action_watchers: AtomicUsize,
    /// The database that everything is also written to, when it is kept.
    store: OnceLock<Store>,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    /// A hub with no printers. [`Hub::add_printer`] gives it some.
    pub fn new() -> Self {
        Self {
            printers: RwLock::new(Vec::new()),
            shelf: Mutex::new(Shelf {
                receipts: VecDeque::new(),
                next_id: 1,
                pictures: Pictures::default(),
            }),
            log: Mutex::new(Log {
                connections: VecDeque::new(),
                next_id: 1,
            }),
            notices: broadcast::channel(4096).0,
            action_watchers: AtomicUsize::new(0),
            store: OnceLock::new(),
        }
    }

    /// Starts writing everything that changes to the database: the address of the API now, and
    /// the printers, the receipts and the connections as they change.
    pub(crate) fn keep_state_in(&self, store: Store, http: std::net::SocketAddr) {
        if self.store.set(store).is_err() {
            return;
        }
        self.write(Command::Http(http));
        self.save_state();
    }

    /// Hands a change to the database, if there is one.
    fn write(&self, command: Command) {
        if let Some(store) = self.store.get() {
            store.send(command);
        }
    }

    /// Writes the printers with their settings and status.
    fn save_state(&self) {
        if self.store.get().is_some() {
            let printers = self
                .printers()
                .iter()
                .map(|printer| SavedPrinter::of(printer))
                .collect();
            self.write(Command::Printers(printers));
        }
    }

    /// Waits until everything is written to the database, and writes nothing after that.
    pub fn close_store(&self) {
        if let Some(store) = self.store.get() {
            store.close();
        }
    }

    /// Puts back the receipts and the connections that the database had. A receipt is read again
    /// from the bytes it was made from, by the printer that printed it, or by what is left of it
    /// when that printer is gone.
    pub(crate) fn restore_history(&self, loaded: Loaded) {
        let mut gone: HashMap<String, Arc<VirtualPrinter>> = HashMap::new();
        let mut shelf = self.shelf();
        for saved in loaded.receipts {
            let Some(dialect) = Dialect::from_name(&saved.dialect) else {
                eprintln!(
                    "stubprint: receipt {} is left out: it has no dialect",
                    saved.id
                );
                continue;
            };
            let profile = Arc::new(Profile::named(&saved.profile).unwrap_or_else(Profile::generic));
            let printer = self
                .printer(&saved.printer)
                .or_else(|| gone.get(&saved.printer).cloned())
                .unwrap_or_else(|| {
                    let printer = Arc::new(VirtualPrinter::new(
                        saved.printer.clone(),
                        dialect,
                        (*profile).clone(),
                        ([127, 0, 0, 1], 0).into(),
                        0,
                        0,
                    ));
                    gone.insert(saved.printer.clone(), printer.clone());
                    printer
                });
            let mut reader = Printer::with_dialect(dialect, (*profile).clone());
            let mut receipts = reader.push_traced(&saved.raw).receipts;
            receipts.extend(reader.finish_traced().receipts);
            let Some(receipt) = receipts.into_iter().next() else {
                eprintln!(
                    "stubprint: receipt {} is left out: it reads as nothing",
                    saved.id
                );
                continue;
            };
            shelf.receipts.push_back(Arc::new(Stored {
                id: saved.id,
                printer,
                profile,
                at: saved.at,
                receipt,
            }));
        }
        shelf.next_id = shelf.next_id.max(loaded.next_receipt);
        drop(shelf);
        let mut log = self.log();
        log.connections.extend(loaded.connections);
        log.next_id = log.next_id.max(loaded.next_connection);
    }

    pub fn printers(&self) -> Vec<Arc<VirtualPrinter>> {
        self.printers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn printer(&self, name: &str) -> Option<Arc<VirtualPrinter>> {
        self.printers()
            .into_iter()
            .find(|printer| printer.name == name)
    }

    /// Opens the port of a printer and starts taking its connections. Fails with
    /// `InvalidInput` for a name that cannot be used, `AlreadyExists` for a name or an address that
    /// is taken, and the error of the system for a port that cannot be opened.
    pub async fn add_printer(
        self: &Arc<Self>,
        settings: PrinterConfig,
    ) -> io::Result<Arc<VirtualPrinter>> {
        PrinterConfig::check_name(&settings.name)
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        if self.printer(&settings.name).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("there is already a printer called {}", settings.name),
            ));
        }
        let socket = listener::open(settings.listen, settings.network_buffer).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "printer {} cannot listen on {}: {error}",
                    settings.name, settings.listen
                ),
            )
        })?;
        let printer = Arc::new(VirtualPrinter::new(
            settings.name,
            settings.dialect,
            settings.profile,
            socket.local_addr()?,
            settings.printer_buffer,
            settings.network_buffer,
        ));
        {
            let mut printers = self
                .printers
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // Another request may have taken the name while the port was being opened.
            if printers.iter().any(|other| other.name == printer.name) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("there is already a printer called {}", printer.name),
                ));
            }
            printers.push(printer.clone());
        }
        printer.listening(tokio::spawn(listener::listen(
            self.clone(),
            printer.clone(),
            socket,
        )));
        self.notify(Notice::PrinterAdded(PrinterInfo::of(&printer)));
        Ok(printer)
    }

    /// The name to give a printer that is asked for `wanted`: the same when no printer has it, and
    /// else with `_2`, `_3` and so on after it, the first that is free.
    pub fn free_name(&self, wanted: &str) -> String {
        let taken = |name: &str| self.printer(name).is_some();
        if !taken(wanted) {
            return wanted.to_string();
        }
        (2..)
            .map(|number| format!("{wanted}_{number}"))
            .find(|name| !taken(name))
            .expect("there is a number that is free")
    }

    /// Closes a printer's port and its connections. The receipts it printed are kept. Returns
    /// whether there was such a printer.
    pub async fn remove_printer(&self, name: &str) -> bool {
        let removed = {
            let mut printers = self
                .printers
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            printers
                .iter()
                .position(|printer| printer.name == name)
                .map(|at| printers.remove(at))
        };
        let Some(printer) = removed else {
            return false;
        };
        printer.shut().await;
        self.notify(Notice::PrinterRemoved {
            name: name.to_string(),
        });
        true
    }

    /// Closes every port and every connection, and waits until they are done.
    pub async fn stop_printers(&self) {
        let printers = std::mem::take(
            &mut *self
                .printers
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for printer in printers {
            printer.shut().await;
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    pub fn notify(&self, notice: Notice) {
        // What a printer is, and what it reports, is written to the database.
        if matches!(
            notice,
            Notice::PrinterAdded(_)
                | Notice::PrinterChanged(_)
                | Notice::PrinterRemoved { .. }
                | Notice::Status { .. }
        ) {
            self.save_state();
        }
        // Nobody listening is fine.
        let _ = self.notices.send(notice);
    }

    fn shelf(&self) -> MutexGuard<'_, Shelf> {
        self.shelf
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts watching actions. They stop being made when the watcher is dropped.
    pub fn watch_actions(self: &Arc<Self>) -> ActionWatch {
        self.action_watchers.fetch_add(1, Ordering::Relaxed);
        ActionWatch(self.clone())
    }

    /// Whether anyone wants the actions, so a job that nobody watches costs nothing.
    pub fn actions_wanted(&self) -> bool {
        self.action_watchers.load(Ordering::Relaxed) > 0
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        self.log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Notes that a program connected. Returns the id of the connection.
    pub fn open_connection(&self, printer: &str, peer: &str) -> u64 {
        let mut log = self.log();
        let id = log.next_id;
        log.next_id += 1;
        let connection = Connection {
            id,
            printer: printer.to_string(),
            peer: peer.to_string(),
            opened_at: now(),
            closed_at: None,
            bytes_in: 0,
            bytes_out: 0,
            chunks: Vec::new(),
            receipts: Vec::new(),
        };
        let opened = ConnectionChange::of(&connection, None);
        log.connections.push_back(connection);
        drop(log);
        self.notify(Notice::Connection(opened));
        id
    }

    /// Changes a connection and tells the watchers what it is now. `change` returns the piece of
    /// bytes it added, if it added one.
    fn update(&self, id: u64, change: impl FnOnce(&mut Connection) -> Option<Chunk>) {
        let changed = self
            .log()
            .connections
            .iter_mut()
            .rev()
            .find(|c| c.id == id)
            .map(|connection| {
                let chunk = change(connection);
                ConnectionChange::of(connection, chunk)
            });
        if let Some(changed) = changed {
            self.notify(Notice::Connection(changed));
        }
    }

    /// Notes bytes that arrived on a connection.
    pub fn received(&self, id: u64, bytes: usize) {
        self.update(id, |connection| {
            let at = now().saturating_sub(connection.opened_at);
            connection.bytes_in += bytes;
            if connection.chunks.len() >= MAX_CHUNKS {
                if let Some(last) = connection.chunks.last_mut() {
                    last.bytes += bytes;
                }
                None
            } else {
                let chunk = Chunk { at, bytes };
                connection.chunks.push(chunk.clone());
                Some(chunk)
            }
        });
    }

    /// Notes bytes the printer answered with.
    pub fn answered(&self, id: u64, bytes: usize) {
        if bytes == 0 {
            return;
        }
        self.update(id, |connection| {
            connection.bytes_out += bytes;
            None
        });
    }

    pub fn close_connection(&self, id: u64) {
        self.update(id, |connection| {
            connection.closed_at = Some(now());
            None
        });
        if self.store.get().is_some() {
            if let Some(connection) = self.connection(id) {
                self.write(Command::Connection(connection));
            }
        }
    }

    /// The connections remembered, oldest first, of one printer or of all.
    pub fn connections(&self, printer: Option<&str>) -> Vec<Connection> {
        self.log()
            .connections
            .iter()
            .filter(|connection| printer.is_none_or(|name| connection.printer == name))
            .cloned()
            .collect()
    }

    pub fn connection(&self, id: u64) -> Option<Connection> {
        self.log()
            .connections
            .iter()
            .rev()
            .find(|c| c.id == id)
            .cloned()
    }

    /// Prints what programs that have gone left in a printer, if a printer in `status` can print.
    /// They print in the order they came, as the printer reads its buffer, and each receipt belongs
    /// to the connection that sent it.
    pub fn release_held(&self, printer: &Arc<VirtualPrinter>, status: PrinterStatus) {
        if status.halts_printing(printer.dialect) {
            return;
        }
        for held in printer.take_held() {
            let mut reader = Printer::with_dialect(printer.dialect, (*held.profile).clone());
            reader.set_status(status);
            let mut receipts = reader.push_traced(&held.bytes).receipts;
            receipts.extend(reader.finish_traced().receipts);
            for receipt in receipts {
                self.store(printer, held.profile.clone(), held.connection, receipt);
            }
        }
        self.announce_held(printer);
    }

    /// Tells the watchers how many bytes a printer holds, when that changed.
    pub fn announce_held(&self, printer: &VirtualPrinter) {
        if let Some(held) = printer.held_changed() {
            self.notify(Notice::Held {
                printer: printer.name.clone(),
                held,
            });
        }
    }

    /// Keeps a receipt and tells the watchers. Returns its id. `connection` is the one that
    /// printed it.
    pub fn store(
        &self,
        printer: &Arc<VirtualPrinter>,
        profile: Arc<Profile>,
        connection: u64,
        receipt: Receipt,
    ) -> u64 {
        let at = now();
        let stored = {
            let mut shelf = self.shelf();
            let id = shelf.next_id;
            shelf.next_id += 1;
            let stored = Arc::new(Stored {
                id,
                printer: printer.clone(),
                profile,
                at,
                receipt,
            });
            shelf.receipts.push_back(stored.clone());
            stored
        };
        self.write(Command::Receipt(SavedReceipt {
            id: stored.id,
            printer: stored.printer.name.clone(),
            dialect: stored.printer.dialect.name().to_string(),
            profile: stored.profile.name.clone(),
            at: stored.at,
            raw: stored.receipt.raw().to_vec(),
        }));
        self.update(connection, |c| {
            c.receipts.push(stored.id);
            None
        });
        self.notify(Notice::Receipt(ReceiptSummary::of(&stored)));
        stored.id
    }

    /// The receipts kept, oldest first, of one printer or of all.
    pub fn receipts(&self, printer: Option<&str>) -> Vec<Arc<Stored>> {
        self.shelf()
            .receipts
            .iter()
            .filter(|stored| printer.is_none_or(|name| stored.printer.name == name))
            .cloned()
            .collect()
    }

    pub fn receipt(&self, id: u64) -> Option<Arc<Stored>> {
        self.shelf()
            .receipts
            .iter()
            .find(|stored| stored.id == id)
            .cloned()
    }

    /// A picture that was drawn before, if it is still kept.
    pub fn picture(&self, key: &PictureKey) -> Option<Bytes> {
        self.shelf().pictures.get(key)
    }

    /// Keeps a picture that was drawn, unless its receipt is gone by now.
    pub fn keep_picture(&self, key: PictureKey, file: Bytes) {
        let mut shelf = self.shelf();
        if shelf.receipts.iter().any(|stored| stored.id == key.0) {
            shelf.pictures.put(key, file);
        }
    }

    /// Forgets one receipt. Its number is not used again.
    pub fn remove_receipt(&self, id: u64) -> bool {
        let removed = {
            let mut shelf = self.shelf();
            let before = shelf.receipts.len();
            shelf.receipts.retain(|stored| stored.id != id);
            shelf.pictures.forget(|gone| gone == id);
            before != shelf.receipts.len()
        };
        if removed {
            self.write(Command::RemoveReceipt(id));
            self.notify(Notice::ReceiptRemoved { id });
        }
        removed
    }

    /// Forgets the receipts of one printer or of all, and returns how many.
    pub fn clear(&self, printer: Option<&str>) -> usize {
        let removed = {
            let mut shelf = self.shelf();
            let before = shelf.receipts.len();
            shelf
                .receipts
                .retain(|stored| printer.is_some_and(|name| stored.printer.name != name));
            let kept: Vec<u64> = shelf.receipts.iter().map(|stored| stored.id).collect();
            shelf.pictures.forget(|id| !kept.contains(&id));
            before - shelf.receipts.len()
        };
        self.write(Command::ClearReceipts(printer.map(str::to_string)));
        self.notify(Notice::Cleared {
            printer: printer.map(str::to_string),
        });
        removed
    }

    fn after(&self, id: u64, printer: Option<&str>) -> Option<Arc<Stored>> {
        self.shelf()
            .receipts
            .iter()
            .find(|stored| stored.id > id && printer.is_none_or(|name| stored.printer.name == name))
            .cloned()
    }

    fn last_id(&self) -> u64 {
        self.shelf().next_id - 1
    }

    /// Waits for the first receipt after the one with id `after`, or after the last one there is
    /// now, and gives up after `timeout`.
    pub async fn next(
        &self,
        after: Option<u64>,
        printer: Option<&str>,
        timeout: Duration,
    ) -> Option<Arc<Stored>> {
        // Listening comes first, so a receipt that arrives while we look is not missed.
        let mut notices = self.subscribe();
        let after = after.unwrap_or_else(|| self.last_id());
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(stored) = self.after(after, printer) {
                return Some(stored);
            }
            match tokio::time::timeout_at(deadline, notices.recv()).await {
                Err(_) | Ok(Err(broadcast::error::RecvError::Closed)) => return None,
                Ok(_) => {}
            }
        }
    }
}

/// Milliseconds since 1970.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// Keeps the actions being made while it is held.
pub struct ActionWatch(Arc<Hub>);

impl Drop for ActionWatch {
    fn drop(&mut self) {
        self.0.action_watchers.fetch_sub(1, Ordering::Relaxed);
    }
}
