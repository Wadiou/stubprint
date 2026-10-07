use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use receipt::{PrinterStatus, Receipt};
use serde::Serialize;
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
        printer: String,
        peer: String,
        open: bool,
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
        }
    }
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
    notices: broadcast::Sender<Notice>,
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
            notices: broadcast::channel(256).0,
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

    /// Keeps a receipt and tells the watchers. Returns its id.
    pub fn store(&self, printer: &Arc<VirtualPrinter>, receipt: Receipt) -> u64 {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
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
