use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use stubprint_core::{Dialect, PrinterStatus, Profile};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Bytes a printer received while it could not print, from a program that has gone. They stay in
/// the printer and print when it can.
#[derive(Debug)]
pub struct Held {
    /// The connection that sent them.
    pub connection: u64,
    /// The model it read as then.
    pub profile: Arc<Profile>,
    pub bytes: Vec<u8>,
}

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
    /// How many bytes it holds while it cannot print. 0 is none.
    hold_offline: AtomicUsize,
    /// How many bytes the network keeps for what a program sends. 0 leaves it to the system.
    receive_buffer: AtomicUsize,
    /// What programs that have gone left in it.
    held: Mutex<Vec<Held>>,
    /// How many bytes the connections that are open hold.
    session_held: AtomicUsize,
    /// How many bytes it held when the watchers were last told.
    told_held: AtomicUsize,
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
        hold_offline: usize,
        receive_buffer: usize,
    ) -> Self {
        Self {
            name,
            dialect,
            profile: Mutex::new(Arc::new(profile)),
            listen,
            status: watch::channel(PrinterStatus::default()).0,
            hold_offline: AtomicUsize::new(hold_offline),
            receive_buffer: AtomicUsize::new(receive_buffer),
            held: Mutex::new(Vec::new()),
            session_held: AtomicUsize::new(0),
            told_held: AtomicUsize::new(0),
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

    /// How many bytes it holds while it cannot print. 0 means it holds nothing and prints whatever
    /// it receives. Connections that are open follow it as it changes.
    pub fn hold_offline(&self) -> usize {
        self.hold_offline.load(Ordering::Relaxed)
    }

    /// Changes how many bytes it holds. Connections that are open hear of it, so what they held
    /// is printed when it is turned off.
    pub fn set_hold_offline(&self, bytes: usize) {
        self.hold_offline.store(bytes, Ordering::Relaxed);
        self.status.send_modify(|_| {});
    }

    /// How many bytes the network keeps for what a program sends before the printer reads it. 0
    /// leaves it to the system.
    pub fn receive_buffer(&self) -> usize {
        self.receive_buffer.load(Ordering::Relaxed)
    }

    /// Changes it for the connections that open from now on.
    pub fn set_receive_buffer(&self, bytes: usize) {
        self.receive_buffer.store(bytes, Ordering::Relaxed);
    }

    /// How many bytes a connection may hold: the size it is set to hold, less what programs that
    /// have gone left in the printer. `None` when it holds nothing.
    pub(crate) fn capacity(&self) -> Option<usize> {
        let hold = self.hold_offline();
        (hold > 0).then(|| hold.saturating_sub(self.held_len()))
    }

    /// How many bytes the printer takes now while it holds what it receives, or `None` when it
    /// holds nothing, because it can print or because it is not set to hold. What programs that
    /// have gone left in it takes room too.
    pub fn room(&self) -> Option<usize> {
        let hold = self.hold_offline();
        (hold > 0 && self.halted()).then(|| hold.saturating_sub(self.held_len()))
    }

    /// How many bytes the printer holds now: what open connections and programs that have gone
    /// left in it.
    pub fn held(&self) -> usize {
        self.held_len() + self.session_held.load(Ordering::Relaxed)
    }

    /// An open connection held `before` bytes and holds `now`.
    pub(crate) fn session_holds(&self, before: usize, now: usize) {
        if now >= before {
            self.session_held.fetch_add(now - before, Ordering::Relaxed);
        } else {
            self.session_held.fetch_sub(before - now, Ordering::Relaxed);
        }
    }

    /// How many bytes the printer holds, when that is not what the watchers were told last.
    pub(crate) fn held_changed(&self) -> Option<usize> {
        let now = self.held();
        (self.told_held.swap(now, Ordering::Relaxed) != now).then_some(now)
    }

    /// How many bytes programs that have gone left in the printer.
    pub(crate) fn held_len(&self) -> usize {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .map(|held| held.bytes.len())
            .sum()
    }

    /// Whether the printer is in a state where it cannot print.
    pub fn halted(&self) -> bool {
        self.status().halts_printing(self.dialect)
    }

    /// Keeps what a program left in the printer when it went.
    pub(crate) fn keep_held(&self, held: Held) {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(held);
    }

    /// Takes what programs that have gone left in the printer, the oldest first.
    pub(crate) fn take_held(&self) -> Vec<Held> {
        std::mem::take(
            &mut *self
                .held
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
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
