use std::sync::Arc;

use crate::bitmap::Bitmap;
use crate::decoder::{Decoder, Traced};
use crate::dialect::Dialect;
use crate::event::Event;
use crate::profile::Profile;
use crate::snapshot::Snapshot;
use crate::status::PrinterStatus;

/// Everything a printer printed between the start of a job and its cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    events: Vec<Event>,
    /// One for each event.
    steps: Vec<Step>,
    raw: Vec<u8>,
}

/// Where the command that made an event ended, and what the printer believed after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// How many bytes of the receipt had been read when the command was done.
    pub end: usize,
    pub state: Arc<Snapshot>,
}

impl Receipt {
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The bytes the printer was sent for this receipt, up to the end of the cut.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// For each event, where its command ended and what the printer believed after it.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The bytes that made one event: from the end of the command before it to the end of its own.
    /// A command that made several events gives each the same bytes, and a text that a style
    /// command ended includes that command.
    pub fn bytes_of(&self, event: usize) -> Option<&[u8]> {
        let end = self.steps.get(event)?.end;
        let start = event
            .checked_sub(1)
            .map_or(0, |before| self.steps[before].end);
        self.raw.get(start.min(end)..end)
    }

    /// The receipt as it was after its first `count` events, as when the paper is looked at in the
    /// middle of the job.
    pub fn until(&self, count: usize) -> Receipt {
        let count = count.min(self.events.len());
        let end = count.checked_sub(1).map_or(0, |last| self.steps[last].end);
        Receipt {
            events: self.events[..count].to_vec(),
            steps: self.steps[..count].to_vec(),
            raw: self.raw[..end.min(self.raw.len())].to_vec(),
        }
    }

    /// The printed lines that are not blank, in order.
    pub fn texts(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Text { text, .. } if !text.is_empty() => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// All the picture bands stacked into one picture, or nothing if no picture was printed.
    pub fn image(&self) -> Option<Bitmap> {
        let bands: Vec<Bitmap> = self
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Image(bitmap) => Some(bitmap.clone()),
                _ => None,
            })
            .collect();
        Bitmap::stack(&bands)
    }
}

/// A blank line or a carriage return after a cut prints nothing, so it is not a receipt.
fn prints_something(events: &[Event]) -> bool {
    events.iter().any(|event| match event {
        Event::Image(_)
        | Event::Barcode(_)
        | Event::Qr(_)
        | Event::Pdf417(_)
        | Event::Symbol(_) => true,
        Event::Text { text, .. } => !text.trim().is_empty(),
        _ => false,
    })
}

/// A fake printer: feed it the bytes a printer would receive, in any number of pieces, and it hands
/// back each receipt as soon as the paper is cut.
#[derive(Debug, Default)]
pub struct Printer {
    decoder: Decoder,
    events: Vec<Event>,
    steps: Vec<Step>,
    /// The bytes received since the end of the last receipt.
    stream: Vec<u8>,
    /// How many bytes came before the first one of `stream`.
    before: usize,
    /// How many bytes it holds while it is off line, when it holds any.
    hold: Option<usize>,
    /// What it received while off line, which prints when it is back on line.
    held: Vec<u8>,
}

impl Printer {
    pub fn new() -> Self {
        Self::default()
    }

    /// A printer of one model, which decides what the numbers of `ESC t` mean and what it says
    /// about itself.
    pub fn with_profile(profile: Profile) -> Self {
        Self::with_dialect(Dialect::EscPos, profile)
    }

    /// A printer of one model that is set to read one command language.
    pub fn with_dialect(dialect: Dialect, profile: Profile) -> Self {
        Self {
            decoder: Decoder::with_dialect(dialect, profile),
            events: Vec::new(),
            steps: Vec::new(),
            stream: Vec::new(),
            before: 0,
            hold: None,
            held: Vec::new(),
        }
    }

    /// Makes the printer hold what it receives while it cannot print, up to `capacity` bytes, as
    /// a real printer keeps it in its receive buffer. It is read and printed when the printer can
    /// print again. `None` makes it read everything at once, whatever its status.
    pub fn hold_while_offline(&mut self, capacity: Option<usize>) {
        self.hold = capacity;
    }

    /// Whether the printer is holding what it receives now.
    fn holding(&self) -> bool {
        self.hold.is_some() && self.status().halts_printing(self.decoder.dialect())
    }

    /// How many more bytes the printer takes while it holds what it receives, so a program that
    /// sends more has to wait. `None` while it prints what it receives, which has no limit.
    pub fn room(&self) -> Option<usize> {
        let capacity = self.hold.filter(|_| self.holding())?;
        Some(capacity.saturating_sub(self.held.len()))
    }

    /// How many bytes the printer holds now.
    pub fn held_len(&self) -> usize {
        self.held.len()
    }

    /// Takes what the printer holds, as when the program that sent it has gone and the bytes stay
    /// in the printer.
    pub fn take_held(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.held)
    }

    /// The bytes the printer answered with since the last call, to send back to the connection.
    pub fn take_replies(&mut self) -> Vec<u8> {
        self.decoder.take_replies()
    }

    pub fn status(&self) -> &PrinterStatus {
        self.decoder.status()
    }

    /// Changes what the printer says about itself, as when the paper runs out. What it held is
    /// read now if it can print again, and what that finished is returned.
    pub fn set_status(&mut self, status: PrinterStatus) -> Pushed {
        self.decoder.set_status(status);
        self.release()
    }

    /// Reads what was held, when the printer can print.
    fn release(&mut self) -> Pushed {
        if self.holding() {
            return Pushed::default();
        }
        self.read(&[])
    }

    /// Reads bytes after the ones that were held.
    fn read(&mut self, bytes: &[u8]) -> Pushed {
        let mut all = std::mem::take(&mut self.held);
        all.extend_from_slice(bytes);
        if all.is_empty() {
            return Pushed::default();
        }
        self.stream.extend_from_slice(&all);
        let traced = self.decoder.push_traced(&all);
        self.absorb(traced)
    }

    /// Adds received bytes and returns the receipts they finished.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Receipt> {
        self.push_traced(bytes).receipts
    }

    /// As [`Printer::push`], and also each event the bytes made, with the place of its command in
    /// the whole stream, so a program can show them while the job is still being sent.
    pub fn push_traced(&mut self, bytes: &[u8]) -> Pushed {
        if !self.holding() {
            return self.read(bytes);
        }
        // The requests for the status are answered as they come. The rest waits.
        let from = self.held.len().saturating_sub(2);
        self.held.extend_from_slice(bytes);
        self.decoder.answer_real_time(&mut self.held, from);
        // An answer may have been a recovery, which lets the printer go on.
        self.release()
    }

    /// Ends the stream, as when the connection closes. A receipt that was not cut is still returned
    /// if it printed something.
    pub fn finish(&mut self) -> Vec<Receipt> {
        self.finish_traced().receipts
    }

    /// As [`Printer::finish`], with the events the end of the stream made.
    pub fn finish_traced(&mut self) -> Pushed {
        let released = self.release();
        let traced = self.decoder.finish_traced();
        let mut pushed = self.absorb(traced);
        pushed.traced.splice(0..0, released.traced);
        pushed.receipts.splice(0..0, released.receipts);
        let events = std::mem::take(&mut self.events);
        let steps = std::mem::take(&mut self.steps);
        let raw = std::mem::take(&mut self.stream);
        self.before += raw.len();
        if prints_something(&events) {
            pushed.receipts.push(Receipt { events, steps, raw });
        }
        pushed
    }

    fn absorb(&mut self, traced: Vec<Traced>) -> Pushed {
        let mut pushed = Pushed {
            traced: Vec::new(),
            receipts: Vec::new(),
        };
        for item in traced {
            let is_cut = matches!(item.event, Event::Cut(_));
            // The ends are counted from the start of the receipt that is being made.
            self.steps.push(Step {
                end: item.end - self.before,
                state: item.state.clone(),
            });
            self.events.push(item.event.clone());
            let end = item.end;
            pushed.traced.push(item);
            if is_cut {
                let events = std::mem::take(&mut self.events);
                let steps = std::mem::take(&mut self.steps);
                if prints_something(&events) {
                    let raw: Vec<u8> = self.stream.drain(..end - self.before).collect();
                    self.before = end;
                    pushed.receipts.push(Receipt { events, steps, raw });
                } else {
                    // What it printed nothing of is kept with the next receipt.
                    self.events = events;
                    self.steps = steps;
                }
            }
        }
        pushed
    }
}

/// What some bytes did: the events they made as they were made, and the receipts they finished.
#[derive(Debug, Default)]
pub struct Pushed {
    pub traced: Vec<Traced>,
    pub receipts: Vec<Receipt>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUT: [u8; 4] = [0x1d, b'V', 0x42, 0x00];

    fn stream() -> Vec<u8> {
        let mut bytes = b"one\n".to_vec();
        bytes.extend(CUT);
        bytes.extend(b"\n\n"); // prints nothing, joins the next receipt
        bytes.extend(b"two\n");
        bytes.extend(CUT);
        bytes.extend(b"three");
        bytes
    }

    fn receipts(piece: usize) -> Vec<Receipt> {
        let mut printer = Printer::new();
        let mut all = Vec::new();
        for chunk in stream().chunks(piece) {
            all.extend(printer.push(chunk));
        }
        all.extend(printer.finish());
        all
    }

    #[test]
    fn each_receipt_keeps_the_bytes_that_made_it_and_together_they_are_the_stream() {
        let all = receipts(1000);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].raw(), [b"one\n".as_slice(), &CUT].concat());
        assert_eq!(all[1].raw(), [b"\n\ntwo\n".as_slice(), &CUT].concat());
        assert_eq!(all[2].raw(), b"three");
        let joined: Vec<u8> = all.iter().flat_map(|r| r.raw().to_vec()).collect();
        assert_eq!(joined, stream());
    }

    #[test]
    fn the_bytes_of_a_receipt_do_not_depend_on_how_the_stream_was_cut() {
        let whole = receipts(1000);
        for piece in [1, 2, 3, 5, 7] {
            assert_eq!(receipts(piece), whole, "pieces of {piece}");
        }
    }

    #[test]
    fn a_cut_that_prints_nothing_gives_no_receipt_and_its_bytes_go_to_the_next() {
        let mut printer = Printer::new();
        assert!(printer.push(&CUT).is_empty());
        let receipts = printer.push(&[b"x\n".as_slice(), &CUT].concat());
        assert_eq!(receipts[0].raw(), [CUT.as_slice(), b"x\n", &CUT].concat());
    }

    #[test]
    fn every_event_has_the_bytes_that_made_it_and_what_the_printer_believed_after() {
        // Bold on, "ab", a line feed, a cut.
        let stream = [b"\x1bE\x01ab\n".as_slice(), &CUT].concat();
        let receipt = &Printer::new().push(&stream)[0];
        let kinds: Vec<String> = receipt
            .events()
            .iter()
            .map(|e| {
                format!("{e:?}")
                    .split([' ', '('])
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(kinds, ["Text", "LineFeed", "Cut"]);
        assert_eq!(receipt.steps().len(), 3);
        // The text and the line feed end where the line feed ends. The cut is the last 4 bytes.
        assert_eq!(receipt.bytes_of(0).unwrap(), b"\x1bE\x01ab\n");
        assert_eq!(receipt.bytes_of(1).unwrap(), b"");
        assert_eq!(receipt.bytes_of(2).unwrap(), CUT);
        assert!(receipt.bytes_of(3).is_none());
        // Bold was on after the first command, and the layout is the default.
        assert!(receipt.steps()[0].state.style.bold);
        assert_eq!(receipt.steps()[2].state.layout, crate::Layout::default());
    }

    #[test]
    fn a_receipt_can_be_looked_at_part_of_the_way_through() {
        let stream = [b"one\ntwo\n".as_slice(), &CUT].concat();
        let receipt = &Printer::new().push(&stream)[0];
        let half = receipt.until(2);
        assert_eq!(half.events().len(), 2);
        assert_eq!(half.texts(), ["one"]);
        assert_eq!(half.raw(), b"one\n");
        assert_eq!(receipt.until(0).raw(), b"");
        assert_eq!(receipt.until(100), *receipt);
    }

    #[test]
    fn what_a_stream_made_is_told_as_it_is_read_and_agrees_with_the_receipts() {
        let stream = [b"one\n".as_slice(), &CUT, b"two\n"].concat();
        let mut printer = Printer::new();
        // "one" is not a line until its line feed arrives.
        let first = printer.push_traced(&stream[..3]);
        assert!(first.traced.is_empty() && first.receipts.is_empty());
        let second = printer.push_traced(&stream[3..]);
        // The ends are counted from the start of the stream, and "two" is a line by now.
        let ends: Vec<usize> = second.traced.iter().map(|t| t.end).collect();
        assert_eq!(ends, [4, 4, 8, 12, 12]);
        assert_eq!(second.receipts.len(), 1);
        assert_eq!(
            second.receipts[0].raw(),
            [b"one\n".as_slice(), &CUT].concat()
        );
        // The last receipt was not cut, and it counts its ends from its own start.
        let last = printer.finish_traced();
        assert!(last.traced.is_empty());
        assert_eq!(last.receipts[0].raw(), b"two\n");
        let own: Vec<usize> = last.receipts[0].steps().iter().map(|s| s.end).collect();
        assert_eq!(own, [4, 4]);
    }

    fn cover_open() -> PrinterStatus {
        PrinterStatus {
            cover_open: true,
            ..PrinterStatus::default()
        }
    }

    #[test]
    fn a_printer_that_does_not_hold_prints_whatever_its_status() {
        let mut printer = Printer::new();
        printer.set_status(cover_open());
        assert_eq!(printer.room(), None);
        let pushed = printer.push_traced(&[b"one\n".as_slice(), &CUT].concat());
        assert_eq!(pushed.receipts.len(), 1);
    }

    #[test]
    fn a_printer_that_holds_keeps_what_it_receives_until_it_can_print_and_then_prints_it_in_order()
    {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(cover_open());
        assert_eq!(printer.room(), Some(64));

        let job = [b"one\n".as_slice(), &CUT, b"two\n", &CUT].concat();
        let pushed = printer.push_traced(&job);
        assert!(pushed.traced.is_empty() && pushed.receipts.is_empty());
        assert_eq!(printer.room(), Some(64 - job.len()));

        let released = printer.set_status(PrinterStatus::default());
        assert_eq!(released.receipts.len(), 2);
        assert_eq!(released.receipts[0].texts(), ["one"]);
        assert_eq!(released.receipts[1].texts(), ["two"]);
        assert_eq!(
            released.receipts[0].raw(),
            [b"one\n".as_slice(), &CUT].concat()
        );
        assert_eq!(printer.room(), None);
        assert!(printer.take_held().is_empty());
    }

    #[test]
    fn the_room_of_a_printer_that_holds_runs_out_and_never_goes_below_nothing() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(4));
        printer.set_status(cover_open());
        printer.push_traced(b"abcdef");
        assert_eq!(printer.room(), Some(0));
    }

    #[test]
    fn what_is_held_is_given_up_when_the_program_has_gone() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(cover_open());
        printer.push_traced(b"one\n");
        printer.finish_traced();
        assert_eq!(printer.take_held(), b"one\n");
    }

    #[test]
    fn a_status_request_is_answered_at_once_while_holding_and_is_not_kept() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(cover_open());
        // DLE EOT 2 says why the printer is off line: the cover.
        printer.push_traced(&[b'a', 0x10, 0x04, 0x02, b'b']);
        assert_eq!(printer.take_replies(), [0x12 | 0x04]);
        assert_eq!(printer.take_held(), b"ab");
    }

    #[test]
    fn a_request_that_is_cut_in_two_pieces_is_still_answered_once() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(cover_open());
        printer.push_traced(&[b'a', 0x10]);
        assert!(printer.take_replies().is_empty());
        printer.push_traced(&[0x04, 0x02]);
        assert_eq!(printer.take_replies(), [0x12 | 0x04]);
        assert_eq!(printer.take_held(), b"a");
    }

    #[test]
    fn a_recovery_asked_while_holding_lets_the_printer_print_what_it_held() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(PrinterStatus {
            fault: Some(crate::status::Fault::Recoverable),
            ..PrinterStatus::default()
        });
        let pushed = printer.push_traced(&[b"one\n".as_slice(), &CUT].concat());
        assert!(pushed.receipts.is_empty());
        // DLE ENQ 1 asks the printer to go on after the error.
        let pushed = printer.push_traced(&[0x10, 0x05, 0x01]);
        assert_eq!(pushed.receipts.len(), 1);
        assert_eq!(printer.room(), None);
    }

    #[test]
    fn a_fault_that_does_not_go_away_is_not_cleared_by_a_request_to_recover() {
        let mut printer = Printer::new();
        printer.hold_while_offline(Some(64));
        printer.set_status(PrinterStatus {
            fault: Some(crate::status::Fault::Unrecoverable),
            ..PrinterStatus::default()
        });
        printer.push_traced(&[b"one\n".as_slice(), &CUT, &[0x10, 0x05, 0x01]].concat());
        assert!(printer.room().is_some());
    }

    #[test]
    fn a_star_line_printer_answers_enq_and_eot_while_holding() {
        let mut printer = Printer::with_dialect(Dialect::StarLine, Profile::generic());
        printer.hold_while_offline(Some(64));
        printer.set_status(cover_open());
        printer.push_traced(&[b'a', 0x05, 0x04, b'b']);
        // ENQ says there is another error, which an open cover is. EOT says the paper is adequate.
        assert_eq!(printer.take_replies(), [0x04, 0x10]);
        assert_eq!(printer.take_held(), b"ab");
    }

    #[test]
    fn the_status_that_stops_a_printer_depends_on_the_language() {
        let held = PrinterStatus {
            feed_button: true,
            ..PrinterStatus::default()
        };
        assert!(held.halts_printing(Dialect::EscPos));
        assert!(!held.halts_printing(Dialect::StarLine));
        for stops in [
            PrinterStatus {
                online: false,
                ..PrinterStatus::default()
            },
            PrinterStatus {
                paper: crate::status::Paper::End,
                ..PrinterStatus::default()
            },
            cover_open(),
        ] {
            assert!(stops.halts_printing(Dialect::EscPos));
            assert!(stops.halts_printing(Dialect::StarPrnt));
        }
        let near_end = PrinterStatus {
            paper: crate::status::Paper::NearEnd,
            ..PrinterStatus::default()
        };
        assert!(!near_end.halts_printing(Dialect::EscPos));
        assert!(!PrinterStatus::default().halts_printing(Dialect::EscPos));
    }
}
