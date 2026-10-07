//! A command language: the way the bytes a program sends ask a printer to do things. ESC/POS and
//! the two languages of Star each read the same bytes differently, so a decoder is set to one, as a
//! port is, and the language works on the printer's shared parts.

use super::escpos::EscPos;
use super::machine::{CodePages, Machine};
use super::reader::{Input, Step};
use super::star::Star;
use crate::dialect::Dialect;
use crate::event::Event;
use crate::status::PrinterStatus;

pub(super) trait Language: std::fmt::Debug {
    /// Reads the command at the start of the bytes, or the text up to the next command, and does
    /// what it says to the machine. An unfinished command asks for more bytes.
    fn parse(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step;

    /// What a number of the command that picks a code page means.
    fn code_pages(&self) -> CodePages;

    /// The status of the printer was changed from outside, as when the paper runs out. A language
    /// that has the printer say so by itself does it here.
    fn status_changed(&mut self, machine: &mut Machine, before: &PrinterStatus);
}

/// The language that a printer is set to.
pub(super) fn for_dialect(dialect: Dialect) -> Box<dyn Language> {
    match dialect {
        Dialect::EscPos => Box::new(EscPos::default()),
        Dialect::StarLine | Dialect::StarPrnt => Box::new(Star::new(dialect)),
    }
}
