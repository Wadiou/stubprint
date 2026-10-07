use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};

use stubprint_core::{Dialect, PrinterStatus, Profile};
use tokio::sync::watch;

/// A virtual printer: what it is, where it listens, and what it pretends to be going through.
#[derive(Debug)]
pub struct VirtualPrinter {
    pub name: String,
    pub dialect: Dialect,
    pub profile: Profile,
    /// The address it really listens on, which differs from the configured one for port 0.
    pub listen: SocketAddr,
    status: watch::Sender<PrinterStatus>,
    connections: AtomicUsize,
}

impl VirtualPrinter {
    pub(crate) fn new(
        name: String,
        dialect: Dialect,
        profile: Profile,
        listen: SocketAddr,
    ) -> Self {
        Self {
            name,
            dialect,
            profile,
            listen,
            status: watch::channel(PrinterStatus::default()).0,
            connections: AtomicUsize::new(0),
        }
    }

    pub fn status(&self) -> PrinterStatus {
        *self.status.borrow()
    }

    /// Sets what the printer says about itself. Connections that are open hear of it.
    pub fn set_status(&self, status: PrinterStatus) {
        self.status.send_replace(status);
    }

    pub(crate) fn watch_status(&self) -> watch::Receiver<PrinterStatus> {
        self.status.subscribe()
    }

    /// How many programs are connected to it now.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }

    pub(crate) fn connected(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn disconnected(&self) {
        self.connections.fetch_sub(1, Ordering::Relaxed);
    }
}
