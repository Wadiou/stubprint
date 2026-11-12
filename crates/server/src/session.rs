use std::sync::Arc;

use stubprint_core::{Printer, PrinterStatus, Profile, Pushed};

use crate::hub::{Hub, Notice};
use crate::printer::{Held, VirtualPrinter};

/// One program talking to a virtual printer, over a socket or in one request. It has its own
/// reading of the commands, as a real printer has for the one job it is printing, and hands the
/// receipts it finishes to the hub.
pub struct Session {
    hub: Arc<Hub>,
    virtual_printer: Arc<VirtualPrinter>,
    /// The model it read as when it opened.
    profile: Arc<Profile>,
    printer: Printer,
    id: u64,
    /// Where the last command ended, which is where the next starts.
    last_end: usize,
    stored: Vec<u64>,
}

impl Session {
    pub fn open(hub: Arc<Hub>, virtual_printer: Arc<VirtualPrinter>, peer: String) -> Self {
        let profile = virtual_printer.profile();
        let mut printer = Printer::with_dialect(virtual_printer.dialect, (*profile).clone());
        // What programs that have gone left in the printer takes room in the same buffer.
        let hold = virtual_printer.hold_offline();
        printer.hold_while_offline(
            (hold > 0).then(|| hold.saturating_sub(virtual_printer.held_len())),
        );
        printer.set_status(virtual_printer.status());
        virtual_printer.connected();
        let id = hub.open_connection(&virtual_printer.name, &peer);
        Self {
            hub,
            virtual_printer,
            profile,
            printer,
            id,
            last_end: 0,
            stored: Vec::new(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Reads received bytes. Returns what the printer answers.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.hub.received(self.id, bytes.len());
        let pushed = self.printer.push_traced(bytes);
        self.take(pushed)
    }

    /// How many more bytes the printer takes now, when it is holding what it receives because it
    /// cannot print. A program that sends more has to wait. `None` when it has no limit.
    pub fn room(&self) -> Option<usize> {
        self.printer.room()
    }

    /// The printer was told it is now in another state. Returns what it says by itself about it,
    /// and what it held is printed if it can print now.
    pub fn set_status(&mut self, status: PrinterStatus) -> Vec<u8> {
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
        self.take(pushed)
    }

    /// Passes on what some bytes made: each command to those who watch the actions, and each
    /// receipt to the hub. Returns what the printer answered.
    fn take(&mut self, pushed: Pushed) -> Vec<u8> {
        let watched = self.hub.actions_wanted();
        for item in pushed.traced {
            if watched {
                self.hub.notify(Notice::Action {
                    connection: self.id,
                    printer: self.virtual_printer.name.clone(),
                    start: self.last_end,
                    end: item.end,
                    event: Arc::new(item.event),
                    state: item.state,
                });
            }
            self.last_end = item.end;
        }
        self.keep(pushed.receipts);
        let replies = self.printer.take_replies();
        self.hub.answered(self.id, replies.len());
        replies
    }

    fn keep(&mut self, receipts: Vec<stubprint_core::Receipt>) {
        for receipt in receipts {
            self.stored.push(self.hub.store(
                &self.virtual_printer,
                self.profile.clone(),
                self.id,
                receipt,
            ));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A connection that drops without a goodbye still prints what it sent.
        self.finish();
        // What the printer still holds stays in it, and prints when it can.
        let bytes = self.printer.take_held();
        if !bytes.is_empty() {
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
