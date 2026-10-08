use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use stubprint_core::{Dialect, PrinterStatus, Profile};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// A virtual printer: what it is, where it listens, and what it pretends to be going through.
#[derive(Debug)]
pub struct VirtualPrinter {
    pub name: String,
    pub dialect: Dialect,
    /// The printer model, which can be changed while it runs.
    profile: Mutex<Arc<Profile>>,
    /// The address it really listens on, which differs from the configured one for port 0.
    pub listen: SocketAddr,
    status: watch::Sender<PrinterStatus>,
    connections: AtomicUsize,
    /// Tells its port and its connections to close.
    stop: watch::Sender<bool>,
    /// The task that takes its connections.
    task: Mutex<Option<JoinHandle<()>>>,
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
            profile: Mutex::new(Arc::new(profile)),
            listen,
            status: watch::channel(PrinterStatus::default()).0,
            connections: AtomicUsize::new(0),
            stop: watch::channel(false).0,
            task: Mutex::new(None),
        }
    }

    /// The model it is now. A receipt keeps the model it was printed on.
    pub fn profile(&self) -> Arc<Profile> {
        self.profile
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Swaps the model, as if another printer stood on the port. Connections that are open keep
    /// reading as the one they started with.
    pub fn set_profile(&self, profile: Profile) {
        *self
            .profile
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(profile);
    }

    pub fn status(&self) -> PrinterStatus {
        *self.status.borrow()
    }

    /// Sets what the printer says about itself. Connections that are open hear of it.
    pub fn set_status(&self, status: PrinterStatus) {
        self.status.send_replace(status);
    }

    /// Closes when the printer is removed or the server stops.
    pub(crate) fn stopped(&self) -> watch::Receiver<bool> {
        self.stop.subscribe()
    }

    pub(crate) fn listening(&self, task: JoinHandle<()>) {
        *self
            .task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(task);
    }

    /// Closes the port and the connections, and waits until they are done.
    pub(crate) async fn shut(&self) {
        // Not `send`, which keeps nothing when no listener is subscribed yet.
        self.stop.send_replace(true);
        let task = self
            .task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(task) = task {
            let _ = task.await;
        }
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
