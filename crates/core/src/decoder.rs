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

use std::sync::Arc;

use crate::dialect::Dialect;
use crate::event::Event;
use crate::layout::Layout;
use crate::profile::Profile;
use crate::snapshot::Snapshot;
use crate::status::PrinterStatus;

/// An event, and where in the stream the command that made it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Traced {
    pub event: Event,
    /// How many bytes had been read, from the first one ever pushed, when the command was done.
    pub end: usize,
    /// What the printer believed once the command was done. Commands that change nothing share one.
    pub state: Arc<Snapshot>,
}

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
    /// The paper settings the events so far have set.
    layout: Layout,
    /// What the printer believed after the last command that made an event.
    state: Arc<Snapshot>,
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
        let machine = Machine::new(profile, language.code_pages());
        let layout = Layout::default();
        Self {
            dialect,
            state: Arc::new(machine.snapshot(&layout)),
            machine,
            language,
            pending: Vec::new(),
            consumed: 0,
            layout,
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
            .into_iter()
            .map(|t| t.event)
            .collect()
    }

    /// Ends the stream. A line with no line feed is printed, and bytes that never became a whole
    /// command are reported as unknown.
    pub fn finish(&mut self) -> Vec<Event> {
        self.finish_traced().into_iter().map(|t| t.event).collect()
    }

    /// As [`Decoder::push`], and each event says where in the stream the command that made it
    /// ended, counted from the first byte ever pushed.
    pub fn push_traced(&mut self, chunk: &[u8]) -> Vec<Traced> {
        self.run(chunk, false)
    }

    /// As [`Decoder::finish`], with the place in the stream of each event.
    pub fn finish_traced(&mut self) -> Vec<Traced> {
        let mut events = self.run(&[], true);
        let mut last = Vec::new();
        self.machine.flush_line(&mut last);
        let end = self.consumed;
        events.extend(self.trace(last, end));
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

    /// Answers the requests that a printer reads as soon as it receives them, and takes them out of
    /// the bytes it holds. Only what starts at `from` or later is new.
    pub fn answer_real_time(&mut self, held: &mut Vec<u8>, from: usize) {
        self.language
            .answer_real_time(&mut self.machine, held, from);
    }

    /// Changes what the printer counted about its own work, as it is when the program asks.
    pub fn set_counters(&mut self, counters: crate::counters::Counters) {
        self.machine.counters = counters;
    }

    /// Changes what the printer says about itself, as when the paper runs out. A language that has
    /// the printer report such a change by itself adds the report to the replies.
    pub fn set_status(&mut self, status: PrinterStatus) {
        let before = std::mem::replace(&mut self.machine.status, status);
        self.language.status_changed(&mut self.machine, &before);
    }

    fn run(&mut self, chunk: &[u8], at_end: bool) -> Vec<Traced> {
        let mut bytes = std::mem::take(&mut self.pending);
        bytes.extend_from_slice(chunk);

        let mut traced = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let input = Input {
                bytes: &bytes[position..],
                offset: self.consumed + position,
                at_end,
            };
            let mut events = Vec::new();
            let step = self.parse(&input, &mut events);
            if let Step::Consumed(length) = step {
                position += length;
            }
            let end = self.consumed + position;
            traced.extend(self.trace(events, end));
            if matches!(step, Step::Incomplete) {
                break;
            }
        }

        if at_end && position < bytes.len() {
            let unknown = Event::Unknown {
                offset: self.consumed + position,
                bytes: bytes[position..].to_vec(),
            };
            traced.extend(self.trace(vec![unknown], self.consumed + bytes.len()));
            position = bytes.len();
        }
        self.consumed += position;
        self.pending = bytes.split_off(position);
        traced
    }

    /// Marks the events one command made with where it ended and what the printer believed after.
    fn trace(&mut self, events: Vec<Event>, end: usize) -> Vec<Traced> {
        if events.is_empty() {
            return Vec::new();
        }
        for event in &events {
            self.layout.apply(event);
        }
        let now = self.machine.snapshot(&self.layout);
        if *self.state != now {
            self.state = Arc::new(now);
        }
        events
            .into_iter()
            .map(|event| Traced {
                event,
                end,
                state: self.state.clone(),
            })
            .collect()
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
    fn a_line_feed_is_an_event_of_its_own_and_ends_the_line_in_every_language() {
        let kinds = |dialect: Dialect| -> Vec<String> {
            let mut decoder = Decoder::with_dialect(dialect, Profile::generic());
            // "a", bold "b", a line feed, then a blank line. Star turns bold on with `ESC E`.
            let bold: &[u8] = if dialect.is_star() {
                &[ESC, b'E']
            } else {
                &[ESC, b'E', 1]
            };
            let bytes = [b"a", bold, b"b", &[LF, LF]].concat();
            let mut events = decoder.push(&bytes);
            events.extend(decoder.finish());
            events
                .iter()
                .map(|event| match event {
                    Event::Text { text, .. } if text.is_empty() => "blank".to_string(),
                    Event::Text { text, .. } => text.clone(),
                    Event::LineFeed => "lf".to_string(),
                    other => format!("{other:?}"),
                })
                .collect()
        };
        for dialect in [Dialect::EscPos, Dialect::StarLine, Dialect::StarPrnt] {
            assert_eq!(
                kinds(dialect),
                ["a", "b", "lf", "blank", "lf"],
                "{dialect:?}"
            );
        }
    }

    #[test]
    fn each_event_comes_with_what_the_printer_believed_after_its_command() {
        let mut decoder = Decoder::with_profile(Profile::named("TM-T88IV").unwrap());
        // Code page 2, centred, a margin of 100, then a line of text.
        let bytes = [ESC, b't', 2, ESC, b'a', 1, GS, b'L', 100, 0, b'x', LF];
        let mut traced = decoder.push_traced(&bytes);
        traced.extend(decoder.finish_traced());
        // The centring is the first event, and the margin is after it.
        assert!(matches!(
            traced[0].event,
            Event::Align(crate::Align::Center)
        ));
        assert_eq!(traced[0].state.layout.margin, 0);
        let first = &traced[1];
        assert!(matches!(first.event, Event::Margin(100)));
        let state = &first.state;
        assert_eq!(
            (state.code_page_number, state.code_page.as_deref()),
            (2, Some("CP850"))
        );
        assert_eq!(state.align, crate::Align::Center);
        assert_eq!(state.layout.margin, 100);
        assert!(!state.utf8 && !state.page_mode && !state.style.bold);
        // Nothing changed between the text and its line feed, so they share one.
        let (text, feed) = (&traced[2], &traced[3]);
        assert!(Arc::ptr_eq(&text.state, &feed.state));
        assert_eq!(first.state, text.state);
    }

    #[test]
    fn every_event_says_where_the_command_that_made_it_ended_whatever_the_cuts() {
        let bytes = [b"ab\n".as_slice(), &[GS, b'V', 0x42, 0], b"cd"].concat();
        let ends = |piece: usize| -> Vec<(Event, usize)> {
            let mut decoder = Decoder::new();
            let mut all = Vec::new();
            for chunk in bytes.chunks(piece) {
                all.extend(decoder.push_traced(chunk));
            }
            all.extend(decoder.finish_traced());
            all.into_iter().map(|t| (t.event, t.end)).collect()
        };
        let whole = ends(100);
        let cut_end = whole
            .iter()
            .find_map(|(e, end)| matches!(e, Event::Cut(_)).then_some(*end));
        assert_eq!(cut_end, Some(7));
        assert_eq!(whole.last().map(|(_, end)| *end), Some(9));
        for piece in [1, 2, 3, 4] {
            assert_eq!(ends(piece), whole, "pieces of {piece}");
        }
    }

    #[test]
    fn a_star_printer_reports_its_status_by_itself_when_it_was_asked_to() {
        use crate::status::Paper;
        let mut decoder = Decoder::with_dialect(Dialect::StarLine, Profile::generic());
        let out = PrinterStatus {
            paper: Paper::End,
            ..PrinterStatus::default()
        };
        // Nothing is sent until the program asks for the status to be sent when it changes.
        decoder.set_status(out);
        assert!(decoder.take_replies().is_empty());
        decoder.set_status(PrinterStatus::default());
        decoder.push(&[ESC, 0x1e, b'a', 1]);
        decoder.set_status(out);
        assert_eq!(
            decoder.take_replies(),
            [0x23, 0x06, 0x08, 0, 0, 0x0c, 0, 0, 0]
        );
        // The same status again is no change.
        decoder.set_status(out);
        assert!(decoder.take_replies().is_empty());
    }

    #[test]
    fn the_same_bytes_mean_what_the_language_of_the_printer_says() {
        // ESC E turns bold on in both, but ENQ is a request in Star Line mode and nothing in ESC/POS.
        let bytes = [0x05, b'x', LF];
        let mut star = Decoder::with_dialect(Dialect::StarLine, Profile::generic());
        star.push(&bytes);
        assert_eq!(star.take_replies(), [0x00]);
        let mut epson = Decoder::new();
        let events = epson.push(&bytes);
        assert!(epson.take_replies().is_empty());
        assert!(events.iter().any(|e| matches!(e, Event::Unknown { .. })));
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
