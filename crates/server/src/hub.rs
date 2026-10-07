use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use stubprint_core::{Event, PrinterStatus, Receipt, Snapshot};
use tokio::sync::broadcast;
use tokio::time::Instant;
use utoipa::ToSchema;

use crate::printer::VirtualPrinter;
use crate::wire::ReceiptSummary;

/// A receipt the server keeps, and where it came from.
#[derive(Debug)]
pub struct Stored {
    pub id: u64,
    pub printer: Arc<VirtualPrinter>,
    /// When it was cut, in milliseconds since 1970.
    pub at: u64,
    pub receipt: Receipt,
}

/// One piece of a connection's bytes as it arrived.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Chunk {
    /// Milliseconds after the connection opened.
    pub at: u64,
    pub bytes: usize,
}

/// How many pieces of one connection are told apart. Later ones are added to the last.
const MAX_CHUNKS: usize = 10_000;
/// How many connections are remembered.
const MAX_CONNECTIONS: usize = 200;

/// One program talking to a printer: when, how, and what came of it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Connection {
    pub id: u64,
    pub printer: String,
    /// The address of the program, or `http` for a request to print.
    pub peer: String,
    /// Milliseconds since 1970.
    pub opened_at: u64,
    pub closed_at: Option<u64>,
    pub bytes_in: usize,
    pub bytes_out: usize,
    pub chunks: Vec<Chunk>,
    /// The receipts it printed.
    pub receipts: Vec<u64>,
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
    Status {
        printer: String,
        status: PrinterStatus,
    },
    Connection {
        id: u64,
        printer: String,
        peer: String,
        open: bool,
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
            Notice::Status { .. } => "status",
            Notice::Connection { .. } => "connection",
            Notice::Action { .. } => "action",
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
}

/// What every connection and every request shares: the printers, the receipts they printed and
/// the notices about it.
#[derive(Debug)]
pub struct Hub {
    printers: Vec<Arc<VirtualPrinter>>,
    keep: usize,
    shelf: Mutex<Shelf>,
    log: Mutex<Log>,
    notices: broadcast::Sender<Notice>,
    /// How many watchers want every action, so they are only made when someone looks.
    action_watchers: AtomicUsize,
}

impl Hub {
    pub fn new(printers: Vec<Arc<VirtualPrinter>>, keep: usize) -> Self {
        Self {
            printers,
            keep,
            shelf: Mutex::new(Shelf {
                receipts: VecDeque::new(),
                next_id: 1,
            }),
            log: Mutex::new(Log {
                connections: VecDeque::new(),
                next_id: 1,
            }),
            notices: broadcast::channel(4096).0,
            action_watchers: AtomicUsize::new(0),
        }
    }

    pub fn printers(&self) -> &[Arc<VirtualPrinter>] {
        &self.printers
    }

    pub fn printer(&self, name: &str) -> Option<&Arc<VirtualPrinter>> {
        self.printers.iter().find(|printer| printer.name == name)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    pub fn notify(&self, notice: Notice) {
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
        log.connections.push_back(Connection {
            id,
            printer: printer.to_string(),
            peer: peer.to_string(),
            opened_at: now(),
            closed_at: None,
            bytes_in: 0,
            bytes_out: 0,
            chunks: Vec::new(),
            receipts: Vec::new(),
        });
        while log.connections.len() > MAX_CONNECTIONS {
            log.connections.pop_front();
        }
        id
    }

    fn with_connection(&self, id: u64, change: impl FnOnce(&mut Connection)) {
        if let Some(connection) = self.log().connections.iter_mut().rev().find(|c| c.id == id) {
            change(connection);
        }
    }

    /// Notes bytes that arrived on a connection.
    pub fn received(&self, id: u64, bytes: usize) {
        self.with_connection(id, |connection| {
            let at = now().saturating_sub(connection.opened_at);
            connection.bytes_in += bytes;
            if connection.chunks.len() >= MAX_CHUNKS {
                if let Some(last) = connection.chunks.last_mut() {
                    last.bytes += bytes;
                }
            } else {
                connection.chunks.push(Chunk { at, bytes });
            }
        });
    }

    /// Notes bytes the printer answered with.
    pub fn answered(&self, id: u64, bytes: usize) {
        self.with_connection(id, |connection| connection.bytes_out += bytes);
    }

    pub fn close_connection(&self, id: u64) {
        self.with_connection(id, |connection| connection.closed_at = Some(now()));
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
        self.log().connections.iter().find(|c| c.id == id).cloned()
    }

    /// Keeps a receipt and tells the watchers. Returns its id. `connection` is the one that
    /// printed it.
    pub fn store(&self, printer: &Arc<VirtualPrinter>, connection: u64, receipt: Receipt) -> u64 {
        let at = now();
        let stored = {
            let mut shelf = self.shelf();
            let id = shelf.next_id;
            shelf.next_id += 1;
            let stored = Arc::new(Stored {
                id,
                printer: printer.clone(),
                at,
                receipt,
            });
            shelf.receipts.push_back(stored.clone());
            while shelf.receipts.len() > self.keep {
                shelf.receipts.pop_front();
            }
            stored
        };
        self.with_connection(connection, |c| c.receipts.push(stored.id));
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

    /// Forgets the receipts of one printer or of all, and returns how many.
    pub fn clear(&self, printer: Option<&str>) -> usize {
        let removed = {
            let mut shelf = self.shelf();
            let before = shelf.receipts.len();
            shelf
                .receipts
                .retain(|stored| printer.is_some_and(|name| stored.printer.name != name));
            before - shelf.receipts.len()
        };
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
