use std::sync::Arc;

use receipt::{Printer, PrinterStatus};

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
    stored: Vec<u64>,
}

impl Session {
    pub fn open(hub: Arc<Hub>, virtual_printer: Arc<VirtualPrinter>, peer: String) -> Self {
        let mut printer =
            Printer::with_dialect(virtual_printer.dialect, virtual_printer.profile.clone());
        printer.set_status(virtual_printer.status());
        virtual_printer.connected();
        hub.notify(Notice::Connection {
            printer: virtual_printer.name.clone(),
            peer: peer.clone(),
            open: true,
        });
        Self {
            hub,
            virtual_printer,
            printer,
            peer,
            stored: Vec::new(),
        }
    }

    /// Reads received bytes. Returns what the printer answers.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let receipts = self.printer.push(bytes);
        self.keep(receipts);
        self.printer.take_replies()
    }

    /// The printer was told it is now in another state. Returns what it says by itself about it.
    pub fn set_status(&mut self, status: PrinterStatus) -> Vec<u8> {
        self.printer.set_status(status);
        self.printer.take_replies()
    }

    /// The ids of the receipts this session printed.
    pub fn stored(&self) -> &[u64] {
        &self.stored
    }

    /// Ends the job. What was not cut yet is printed if it has something on it. Returns the last
    /// answers.
    pub fn finish(&mut self) -> Vec<u8> {
        let receipts = self.printer.finish();
        self.keep(receipts);
        self.printer.take_replies()
    }

    fn keep(&mut self, receipts: Vec<receipt::Receipt>) {
        for receipt in receipts {
            self.stored
                .push(self.hub.store(&self.virtual_printer, receipt));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A connection that drops without a goodbye still prints what it sent.
        self.finish();
        self.virtual_printer.disconnected();
        self.hub.notify(Notice::Connection {
            printer: self.virtual_printer.name.clone(),
            peer: std::mem::take(&mut self.peer),
            open: false,
        });
    }
}
