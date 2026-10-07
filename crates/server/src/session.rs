use std::sync::Arc;

use receipt::{Printer, PrinterStatus, Pushed};

use crate::hub::{Hub, Notice};
use crate::printer::VirtualPrinter;

/// One program talking to a virtual printer, over a socket or in one request. It has its own
/// reading of the commands, as a real printer has for the one job it is printing, and hands the
/// receipts it finishes to the hub.
pub struct Session {
    hub: Arc<Hub>,
    virtual_printer: Arc<VirtualPrinter>,
    printer: Printer,
    peer: String,
    id: u64,
    /// Where the last command ended, which is where the next starts.
    last_end: usize,
    stored: Vec<u64>,
}

impl Session {
    pub fn open(hub: Arc<Hub>, virtual_printer: Arc<VirtualPrinter>, peer: String) -> Self {
        let mut printer =
            Printer::with_dialect(virtual_printer.dialect, virtual_printer.profile.clone());
        printer.set_status(virtual_printer.status());
        virtual_printer.connected();
        let id = hub.open_connection(&virtual_printer.name, &peer);
        hub.notify(Notice::Connection {
            id,
            printer: virtual_printer.name.clone(),
            peer: peer.clone(),
            open: true,
        });
        Self {
            hub,
            virtual_printer,
            printer,
            peer,
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

    /// The printer was told it is now in another state. Returns what it says by itself about it.
    pub fn set_status(&mut self, status: PrinterStatus) -> Vec<u8> {
        self.printer.set_status(status);
        let replies = self.printer.take_replies();
        self.hub.answered(self.id, replies.len());
        replies
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

    fn keep(&mut self, receipts: Vec<receipt::Receipt>) {
        for receipt in receipts {
            self.stored
                .push(self.hub.store(&self.virtual_printer, self.id, receipt));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A connection that drops without a goodbye still prints what it sent.
        self.finish();
        self.virtual_printer.disconnected();
        self.hub.close_connection(self.id);
        self.hub.notify(Notice::Connection {
            id: self.id,
            printer: self.virtual_printer.name.clone(),
            peer: std::mem::take(&mut self.peer),
            open: false,
        });
    }
}
