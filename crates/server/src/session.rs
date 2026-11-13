use std::sync::Arc;

use stubprint_core::{Printer, PrinterStatus, Profile, Pushed};

use crate::engine::{Planner, Work};
use crate::hub::Hub;
use crate::printer::{Held, VirtualPrinter};

/// One program talking to a virtual printer, over a socket or in one request. It has its own
/// reading of the commands, as a real printer has for the one job it is printing. What it reads
/// is planned as work for the printer's engine, which prints it over time.
///
/// A session that is *paced*, as one on a socket is, puts its bytes in the printer's receive buffer
/// and waits when it is full. One that is not, as a request to the API, prints at once.
pub struct Session {
    hub: Arc<Hub>,
    virtual_printer: Arc<VirtualPrinter>,
    /// The model it read as when it opened.
    profile: Arc<Profile>,
    printer: Printer,
    id: u64,
    stored: Vec<u64>,
    /// How many bytes it holds, as the printer was last told.
    held: usize,
    /// Turns the commands it read into work and says how long each takes.
    planner: Planner,
    /// Whether it goes through the receive buffer and waits for the printing.
    paced: bool,
    /// How many bytes it put in the receive buffer.
    received: usize,
}

impl Session {
    pub fn open(hub: Arc<Hub>, virtual_printer: Arc<VirtualPrinter>, peer: String) -> Self {
        let profile = virtual_printer.profile();
        let mut printer = Printer::with_dialect(virtual_printer.dialect, (*profile).clone());
        printer.hold_while_offline(virtual_printer.capacity());
        printer.set_status(virtual_printer.status());
        printer.set_counters(virtual_printer.counters());
        virtual_printer.connected();
        let id = hub.open_connection(&virtual_printer.name, &peer);
        Self {
            hub,
            virtual_printer,
            profile,
            printer,
            id,
            stored: Vec::new(),
            held: 0,
            planner: Planner::default(),
            paced: false,
            received: 0,
        }
    }

    /// Makes the session go through the printer's receive buffer, where what it sends waits for the
    /// printing, and the program waits when the buffer is full.
    pub fn paced(mut self) -> Self {
        self.paced = true;
        self
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Reads received bytes. Returns what the printer answers.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.follow_hold();
        // What the printer has counted, which other connections may have changed.
        self.printer.set_counters(self.virtual_printer.counters());
        self.hub.received(self.id, bytes.len());
        if self.paced {
            self.virtual_printer.receive(bytes.len());
            self.received += bytes.len();
        }
        let pushed = self.printer.push_traced(bytes);
        self.take(pushed)
    }

    /// How long a connection can send nothing before the printer closes it, or `None` for ever.
    pub fn idle_timeout(&self) -> Option<std::time::Duration> {
        match self.virtual_printer.idle_timeout() {
            0 => None,
            seconds => Some(std::time::Duration::from_secs(seconds as u64)),
        }
    }

    /// How many more bytes the printer takes now. A program that sends more has to wait: when the
    /// printer holds what it receives because it cannot print, and when its receive buffer is full of
    /// what it did not print yet. `None` when there is no limit.
    pub fn room(&mut self) -> Option<usize> {
        self.follow_hold();
        let buffer = if self.paced {
            self.virtual_printer.buffer_room()
        } else {
            None
        };
        match (self.printer.room(), buffer) {
            (Some(held), Some(buffer)) => Some(held.min(buffer)),
            (room, None) | (None, room) => room,
        }
    }

    /// Takes the size the printer is set to hold now, which may have changed since this
    /// connection opened.
    fn follow_hold(&mut self) {
        self.printer
            .hold_while_offline(self.virtual_printer.capacity());
    }

    /// The printer was told it is now in another state. Returns what it says by itself about it,
    /// and what it held is printed if it can print now.
    pub fn set_status(&mut self, status: PrinterStatus) -> Vec<u8> {
        self.follow_hold();
        let pushed = self.printer.set_status(status);
        self.take(pushed)
    }

    /// The ids of the receipts this session printed.
    pub fn stored(&self) -> &[u64] {
        &self.stored
    }

    /// Ends the job. What was not cut yet is printed if it has something on it. Returns the last
    /// answers.
    pub fn finish(&mut self) -> Vec<u8> {
        let pushed = self.printer.finish_traced();
        let replies = self.take(pushed);
        self.planner.ended();
        replies
    }

    /// Passes on what some bytes made: the work the printer has to do for them goes to its engine.
    /// Returns what the printer answered.
    fn take(&mut self, pushed: Pushed) -> Vec<u8> {
        let work = self.planner.plan(
            self.id,
            pushed,
            &self.profile,
            self.virtual_printer.print_speed(),
        );
        self.run(work);
        self.sync_held();
        let replies = self.printer.take_replies();
        self.hub.answered(self.id, replies.len());
        replies
    }

    /// Prints the work now, when the printer takes no time and has nothing else to do and when the
    /// session is not paced, and otherwise leaves it to the engine, which takes the time.
    fn run(&mut self, work: Vec<Work>) {
        if self.paced && !self.virtual_printer.is_instant() {
            self.virtual_printer.enqueue(work);
            return;
        }
        if self.paced {
            // The bytes of the commands are printed, so they are out of the buffer.
            self.virtual_printer
                .give_back(work.iter().map(|work| work.freed).sum());
        }
        let finished = self.hub.print(&self.virtual_printer, work);
        self.stored.extend(finished);
    }

    /// Tells the printer, and those who watch, how many bytes this connection holds now.
    fn sync_held(&mut self) {
        let now = self.printer.held_len();
        if now != self.held {
            self.virtual_printer.session_holds(self.held, now);
            self.held = now;
        }
        self.hub.announce_held(&self.virtual_printer);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A connection that drops without a goodbye still prints what it sent.
        self.finish();
        // What it sent after its last command, which no command will free, leaves the buffer.
        if self.paced {
            self.virtual_printer
                .give_back(self.received.saturating_sub(self.planner.last_end()));
        }
        // What the printer still holds stays in it, and prints when it can.
        let bytes = self.printer.take_held();
        // They are the printer's now, and no longer this connection's.
        self.virtual_printer.session_holds(self.held, 0);
        self.held = 0;
        if bytes.is_empty() {
            self.hub.announce_held(&self.virtual_printer);
        } else {
            self.virtual_printer.keep_held(Held {
                connection: self.id,
                profile: self.profile.clone(),
                bytes,
            });
            // The printer may have been set right while the connection closed.
            self.hub
                .release_held(&self.virtual_printer, self.virtual_printer.status());
        }
        self.virtual_printer.disconnected();
        self.hub.close_connection(self.id);
    }
}
