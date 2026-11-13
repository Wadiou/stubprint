//! The clock of a printer: it gives the engine the time that passes while the printer can print,
//! and passes on what was done in it. It is the only place where the engine meets real time, and
//! the hub does what it printed.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::{sleep, Instant};

use crate::hub::Hub;
use crate::printer::VirtualPrinter;

/// Drives the engine of one printer until the printer is stopped.
pub(crate) async fn run(hub: Arc<Hub>, printer: Arc<VirtualPrinter>) {
    let mut stop = printer.stopped();
    let mut status = printer.watch_status();
    let mut last = Instant::now();
    // Whether time was passing since the last look: there was work and the printer could print.
    // Time that passes while the printer idles or is off line is not time spent printing.
    let mut counting = false;
    loop {
        let due = printer.due_in();
        tokio::select! {
            _ = stop.wait_for(|stopped| *stopped) => return,
            _ = status.changed() => {}
            () = printer.woken() => {}
            () = sleep(due.unwrap_or_default()), if due.is_some() => {}
        }
        let now = Instant::now();
        let elapsed = if counting { now - last } else { Duration::ZERO };
        last = now;
        if printer.halted() {
            counting = false;
            continue;
        }
        let done = printer.advance(elapsed);
        counting = printer.due_in().is_some();
        if !done.is_empty() {
            hub.print(&printer, done);
        }
    }
}
