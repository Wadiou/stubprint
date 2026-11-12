use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use stubprint_core::{Counters, Dialect, Paper, PrinterStatus, Profile};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use utoipa::ToSchema;

/// The roll reports near end when this share of it is left, or less.
const NEAR_END_SHARE: f64 = 0.1;

/// A length in millimetres as a number to count with.
fn millimetres(length: usize) -> f64 {
    f64::from(u32::try_from(length).unwrap_or(u32::MAX))
}

/// What a printer has done since it was made, which a real printer counts for its service.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Usage {
    /// How many receipts it printed.
    pub receipts: u64,
    /// How many times the cutter cut.
    pub cuts: u64,
    /// How many lines the paper was fed by line feeds and by feeding lines.
    pub line_feeds: u64,
    /// How much paper it used, in millimetres.
    pub paper_used: f64,
}

/// The paper roll, and what was printed on it.
#[derive(Debug, Clone, Copy, Default)]
struct Roll {
    /// How long the roll was when it was full, in millimetres. 0 is a roll that never runs out.
    length: usize,
    /// How much paper is left on it, in millimetres.
    left: f64,
    usage: Usage,
}

impl Roll {
    /// What the sensors of a roll report with this much left on it.
    fn level(&self) -> Paper {
        if self.left <= 0.0 {
            Paper::End
        } else if self.left <= millimetres(self.length) * NEAR_END_SHARE {
            Paper::NearEnd
        } else {
            Paper::Adequate
        }
    }
}

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
    /// How many bytes it holds while it cannot print. 0 means it holds nothing.
    printer_buffer: AtomicUsize,
    /// How many bytes the network keeps for a program's data. 0 leaves it to the operating system.
    network_buffer: AtomicUsize,
    /// Bytes left behind by programs that disconnected while it could not print. They print when
    /// it can.
    held: Mutex<Vec<Held>>,
    /// Bytes held by the connections that are open now.
    session_held: AtomicUsize,
    /// The `held` total the watchers were last told, so an event is sent only when it changes.
    told_held: AtomicUsize,
    roll: Mutex<Roll>,
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
        printer_buffer: usize,
        network_buffer: usize,
        paper_roll: usize,
    ) -> Self {
        Self {
            name,
            dialect,
            profile: Mutex::new(Arc::new(profile)),
            listen,
            status: watch::channel(PrinterStatus::default()).0,
            printer_buffer: AtomicUsize::new(printer_buffer),
            network_buffer: AtomicUsize::new(network_buffer),
            held: Mutex::new(Vec::new()),
            session_held: AtomicUsize::new(0),
            told_held: AtomicUsize::new(0),
            roll: Mutex::new(Roll {
                length: paper_roll,
                left: millimetres(paper_roll),
                usage: Usage::default(),
            }),
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

    fn roll(&self) -> Roll {
        *self
            .roll
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// How long its paper roll was when it was full, in millimetres. 0 is a roll that never runs
    /// out.
    pub fn paper_roll(&self) -> usize {
        self.roll().length
    }

    /// How much paper is left on the roll, in millimetres.
    pub fn paper_left(&self) -> f64 {
        self.roll().left
    }

    /// What it has done since it was made.
    pub fn usage(&self) -> Usage {
        self.roll().usage
    }

    /// What a program can ask it for as its maintenance counters.
    pub fn counters(&self) -> Counters {
        let usage = self.roll().usage;
        Counters {
            line_feeds: usage.line_feeds,
            cuts: usage.cuts,
        }
    }

    /// What the sensors of the roll report, or `None` for a roll that never runs out and leaves
    /// the paper to be set by hand.
    pub fn roll_level(&self) -> Option<Paper> {
        let roll = self.roll();
        (roll.length > 0).then(|| roll.level())
    }

    /// Takes back what it had before the server stopped: the paper left and what it had done.
    pub(crate) fn restore_roll(&self, left: f64, usage: Usage) {
        let mut roll = self
            .roll
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        roll.left = left.clamp(0.0, millimetres(roll.length));
        roll.usage = usage;
    }

    /// Puts in a new roll, which is full: `length` millimetres, or 0 for a roll that never runs
    /// out. Returns what the sensors report now, or `None` when the paper is left to be set by hand.
    pub fn load_roll(&self, length: usize) -> Option<Paper> {
        let mut roll = self
            .roll
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        roll.length = length;
        roll.left = millimetres(length);
        (length > 0).then(|| roll.level())
    }

    /// The paper was set by hand to `paper`: a roll that runs out is made to match.
    pub fn match_roll(&self, paper: Paper) {
        let mut roll = self
            .roll
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if roll.length == 0 {
            return;
        }
        let length = millimetres(roll.length);
        roll.left = match paper {
            Paper::Adequate => length,
            Paper::NearEnd => length * NEAR_END_SHARE,
            Paper::End => 0.0,
        };
    }

    /// Counts a receipt it printed and takes the paper it used off the roll. Returns what the
    /// sensors report now, when that is not what the printer says already.
    pub(crate) fn printed(&self, used: Usage) -> Option<Paper> {
        let level = {
            let mut roll = self
                .roll
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            roll.usage.receipts += used.receipts;
            roll.usage.cuts += used.cuts;
            roll.usage.line_feeds += used.line_feeds;
            roll.usage.paper_used += used.paper_used;
            if roll.length == 0 {
                return None;
            }
            roll.left = (roll.left - used.paper_used).max(0.0);
            roll.level()
        };
        (level != self.status().paper).then_some(level)
    }

    /// How many bytes it holds while it cannot print. 0 means it holds nothing and prints whatever
    /// it receives. Connections that are open follow it as it changes.
    pub fn printer_buffer(&self) -> usize {
        self.printer_buffer.load(Ordering::Relaxed)
    }

    /// Changes how many bytes it holds. Connections that are open hear of it, so what they held
    /// is printed when it is turned off.
    pub fn set_printer_buffer(&self, bytes: usize) {
        self.printer_buffer.store(bytes, Ordering::Relaxed);
        self.status.send_modify(|_| {});
    }

    /// How many bytes the network keeps for what a program sends before the printer reads it. 0
    /// leaves it to the system.
    pub fn network_buffer(&self) -> usize {
        self.network_buffer.load(Ordering::Relaxed)
    }

    /// Changes it for the connections that open from now on.
    pub fn set_network_buffer(&self, bytes: usize) {
        self.network_buffer.store(bytes, Ordering::Relaxed);
    }

    /// How many bytes a connection may hold, whatever the printer's state: `printer_buffer` minus
    /// what disconnected programs left behind. `None` when `printer_buffer` is 0.
    pub(crate) fn capacity(&self) -> Option<usize> {
        let hold = self.printer_buffer();
        (hold > 0).then(|| hold.saturating_sub(self.held_len()))
    }

    /// How many more bytes the printer will read right now. `None` means no limit, because it can
    /// print or because `printer_buffer` is 0. While it cannot print, it is `printer_buffer` minus
    /// what disconnected programs left behind.
    pub fn room(&self) -> Option<usize> {
        let hold = self.printer_buffer();
        (hold > 0 && self.halted()).then(|| hold.saturating_sub(self.held_len()))
    }

    /// How many bytes the printer holds now: what open connections hold, plus what disconnected
    /// programs left behind.
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

    /// How many bytes disconnected programs left behind.
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
