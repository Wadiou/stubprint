//! Reads the bytes a printer receives and says what they ask for.
//!
//! A decoder works on the parts of a printer that every command language shares, [`machine`], and
//! reads its bytes in one command language, which it is set to: [`escpos`], or one of the two
//! languages of Star in [`star`]. Pictures, two dimensional codes and Kanji, which more than one
//! language asks for, are read by modules of their own.

mod escpos;
mod kanji;
mod language;
mod machine;
mod memory;
mod pictures;
mod reader;
mod star;
mod symbols;

use language::Language;
use machine::Machine;
use reader::Input;
use reader::Step;

use crate::dialect::Dialect;
use crate::event::Event;
use crate::profile::Profile;
use crate::status::PrinterStatus;

/// Reads the bytes a printer would receive and says what they ask for. It accepts the bytes in any
/// number of pieces, since a network or Bluetooth link can cut a command anywhere, and holds on to
/// a half received command until the rest arrives. The same bytes give the same events however
/// they are split.
///
/// A program that talks to the printer may also ask it things, such as whether the paper has run
/// out. The answers are bytes for the connection, and wait in [`Decoder::take_replies`].
#[derive(Debug)]
pub struct Decoder {
    dialect: Dialect,
    language: Box<dyn Language>,
    machine: Machine,
    pending: Vec<u8>,
    /// How many bytes were consumed before the ones in `pending`.
    consumed: usize,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::with_profile(Profile::generic())
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A decoder for one printer model, which decides what the numbers of `ESC t` mean and what
    /// the printer says about itself.
    pub fn with_profile(profile: Profile) -> Self {
        Self::with_dialect(Dialect::EscPos, profile)
    }

    /// A decoder for a printer that is set to read one command language.
    pub fn with_dialect(dialect: Dialect, profile: Profile) -> Self {
        let language = language::for_dialect(dialect);
        Self {
            dialect,
            machine: Machine::new(profile, language.code_pages()),
            language,
            pending: Vec::new(),
            consumed: 0,
        }
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub fn profile(&self) -> &Profile {
        &self.machine.profile
    }

    /// Adds received bytes and returns the events they completed.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Event> {
        self.run(chunk, false)
    }

    /// Ends the stream. A line with no line feed is printed, and bytes that never became a whole
    /// command are reported as unknown.
    pub fn finish(&mut self) -> Vec<Event> {
        let mut events = self.run(&[], true);
        self.machine.flush_line(&mut events);
        events
    }

    /// How many received bytes are waiting for the rest of their command.
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    /// The bytes the printer answered with, since the last call, to send back to the program.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.machine.replies)
    }

    /// What the printer says about itself when it is asked.
    pub fn status(&self) -> &PrinterStatus {
        &self.machine.status
    }

    /// Changes what the printer says about itself, as when the paper runs out. A language that has
    /// the printer report such a change by itself adds the report to the replies.
    pub fn set_status(&mut self, status: PrinterStatus) {
        let before = std::mem::replace(&mut self.machine.status, status);
        self.language.status_changed(&mut self.machine, &before);
    }

    fn run(&mut self, chunk: &[u8], at_end: bool) -> Vec<Event> {
        let mut bytes = std::mem::take(&mut self.pending);
        bytes.extend_from_slice(chunk);

        let mut events = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let input = Input {
                bytes: &bytes[position..],
                offset: self.consumed + position,
                at_end,
            };
            match self.parse(&input, &mut events) {
                Step::Consumed(length) => position += length,
                Step::Incomplete => break,
            }
        }

        if at_end && position < bytes.len() {
            events.push(Event::Unknown {
                offset: self.consumed + position,
                bytes: bytes[position..].to_vec(),
            });
            position = bytes.len();
        }
        self.consumed += position;
        self.pending = bytes.split_off(position);
        events
    }

    /// A byte that answers the list of keys being sent is used up. Any other is for the language.
    fn parse(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        if let Some(answer) = self.machine.memory.answer(input.bytes[0]) {
            self.machine.reply(&answer);
            return Step::Consumed(1);
        }
        self.language.parse(&mut self.machine, input, events)
    }
}

#[cfg(test)]
mod tests {
    use super::reader::{ESC, GS, LF, MAX_BARCODE_BYTES, MAX_LINE_BYTES};
    use super::*;
    use crate::event::Event;

    fn decode(bytes: &[u8], piece: usize) -> (Vec<Event>, usize) {
        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        for chunk in bytes.chunks(piece) {
            events.extend(decoder.push(chunk));
        }
        events.extend(decoder.finish());
        (events, decoder.pending_bytes())
    }

    #[test]
    fn a_character_size_is_never_more_than_8_times() {
        for n in 0..=255u8 {
            let (events, _) = decode(&[GS, b'!', n, b'x', LF], 100);
            for event in events {
                if let Event::Text { style, .. } = event {
                    assert!((1..=8).contains(&style.width), "GS ! {n}");
                    assert!((1..=8).contains(&style.height), "GS ! {n}");
                }
            }
        }
    }

    #[test]
    fn a_picture_that_promises_gigabytes_is_reported_and_not_waited_for() {
        let mut bytes = vec![GS, b'v', b'0', 0, 0xff, 0xff, 0xff, 0xff];
        bytes.extend_from_slice(b"after\n");
        // Before the stream ends, nothing is left waiting for the promised bytes.
        let mut decoder = Decoder::new();
        let events = decoder.push(&bytes);
        assert_eq!(decoder.pending_bytes(), 0);
        assert!(matches!(&events[0], Event::Unknown { offset: 0, .. }));
    }

    #[test]
    fn a_barcode_that_never_ends_is_reported_and_not_waited_for() {
        let mut bytes = vec![GS, b'k', 4];
        bytes.extend(std::iter::repeat_n(b'A', MAX_BARCODE_BYTES + 100));
        let mut decoder = Decoder::new();
        let events = decoder.push(&bytes);
        assert_eq!(decoder.pending_bytes(), 0);
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Unknown { offset: 0, .. })));
    }

    #[test]
    fn a_line_with_no_line_feed_is_reported_in_pieces_wherever_it_was_cut() {
        let bytes = vec![b'A'; MAX_LINE_BYTES * 2 + 1808];
        let (whole, _) = decode(&bytes, bytes.len());
        let (single, _) = decode(&bytes, 1);
        let (odd, _) = decode(&bytes, 777);
        assert_eq!(whole, single);
        assert_eq!(whole, odd);
        let lengths: Vec<usize> = whole
            .iter()
            .filter_map(|event| match event {
                Event::Text { text, .. } => Some(text.len()),
                _ => None,
            })
            .collect();
        assert_eq!(lengths, vec![MAX_LINE_BYTES, MAX_LINE_BYTES, 1808]);
    }

    #[test]
    fn the_number_of_a_code_page_means_what_the_printer_profile_says() {
        let bytes = [ESC, b't', 2, 0x80, LF];
        let text = |profile: &str| -> String {
            let mut decoder = Decoder::with_profile(Profile::named(profile).unwrap());
            let mut events = decoder.push(&bytes);
            events.extend(decoder.finish());
            events
                .into_iter()
                .find_map(|event| match event {
                    Event::Text { text, .. } if !text.is_empty() => Some(text),
                    _ => None,
                })
                .unwrap()
        };
        // Number 2 is CP850 on an Epson and CP932 on a Star.
        assert_eq!(text("TM-T88IV"), "Ç");
        assert_eq!(text("SP2000"), "\u{fffd}");
    }
}
