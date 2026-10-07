//! ESC/POS, the command language of Epson, which most printers copy. It is read from the commands
//! that start with `ESC`, `GS` and `FS`, the real time requests that start with `DLE`, and the
//! control bytes.

mod barcode;
mod dle;
mod esc;
mod fs;
mod gs;

use super::language::Language;
use super::machine::{CodePages, Machine};
use super::reader::{Input, Step, BEL, CAN, CR, DLE, ESC, FF, FIRST_TEXT_BYTE, FS, GS, HT, LF};
use crate::event::{Event, Page};
use crate::status::PrinterStatus;

/// What the commands of ESC/POS set, other than what every language sets: the barcode that comes
/// next, and what the printer reports by itself.
#[derive(Debug, Default)]
pub(super) struct EscPos {
    barcode: barcode::Settings,
    /// What `GS a` asked the printer to report on its own.
    automatic_status: u8,
}

impl EscPos {
    /// `ESC @` resets the printer, and every setting returns to its default.
    fn initialize(&mut self, machine: &mut Machine, events: &mut Vec<Event>) {
        machine.initialize(events);
        *self = Self::default();
    }

    /// The status changes by a command, as when the program asks the printer to recover, and the
    /// printer reports it if it was asked to.
    fn set_status(&mut self, machine: &mut Machine, status: PrinterStatus) {
        let before = std::mem::replace(&mut machine.status, status);
        self.status_changed(machine, &before);
    }
}

impl Language for EscPos {
    fn parse(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let bytes = input.bytes;
        match bytes[0] {
            ESC => self.esc(machine, input, events),
            GS => self.gs(machine, input, events),
            FS => self.fs(machine, input, events),
            DLE => self.dle(machine, input, events),
            LF => {
                machine.line_feed(events);
                Step::Consumed(1)
            }
            CR => Step::Consumed(1),
            // A tab stays in the text as a tab, for whoever draws the line to place.
            HT => {
                machine.add_text(&[HT], events);
                Step::Consumed(1)
            }
            BEL => {
                machine.flush_line(events);
                events.push(Event::Beep {
                    times: 1,
                    duration: 0,
                });
                Step::Consumed(1)
            }
            // NUL and BS print nothing.
            0x00 | 0x08 => Step::Consumed(1),
            // FF prints what is in the buffer. In page mode it prints the page too.
            FF => {
                machine.flush_line(events);
                if machine.state.page_mode {
                    machine.state.page_mode = false;
                    events.push(Event::Page(Page::Print));
                    events.push(Event::Page(Page::Standard));
                }
                Step::Consumed(1)
            }
            // CAN throws away what page mode laid out.
            CAN => {
                if machine.state.page_mode {
                    machine.flush_line(events);
                    events.push(Event::Page(Page::Cancel));
                }
                Step::Consumed(1)
            }
            byte if byte >= FIRST_TEXT_BYTE => {
                let length = bytes
                    .iter()
                    .position(|byte| *byte < FIRST_TEXT_BYTE)
                    .unwrap_or(bytes.len());
                machine.add_text(&bytes[..length], events);
                Step::Consumed(length)
            }
            _ => input.unknown_run(events),
        }
    }

    fn code_pages(&self) -> CodePages {
        |profile, number| profile.code_page(number)
    }

    /// If `GS a` asked the printer to report such a change, the report is sent.
    fn status_changed(&mut self, machine: &mut Machine, before: &PrinterStatus) {
        let mask = self.automatic_status;
        if mask != 0 && before.changed(&machine.status, mask) {
            let report = machine.status.automatic();
            machine.reply(&report);
        }
    }
}
