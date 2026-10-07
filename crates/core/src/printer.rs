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
        }
    }

    /// The bytes the printer answered with since the last call, to send back to the connection.
    pub fn take_replies(&mut self) -> Vec<u8> {
        self.decoder.take_replies()
    }

    pub fn status(&self) -> &PrinterStatus {
        self.decoder.status()
    }

    /// Changes what the printer says about itself, as when the paper runs out.
    pub fn set_status(&mut self, status: PrinterStatus) {
        self.decoder.set_status(status);
    }

    /// Adds received bytes and returns the receipts they finished.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Receipt> {
        self.push_traced(bytes).receipts
    }

    /// As [`Printer::push`], and also each event the bytes made, with the place of its command in
    /// the whole stream, so a program can show them while the job is still being sent.
    pub fn push_traced(&mut self, bytes: &[u8]) -> Pushed {
        self.stream.extend_from_slice(bytes);
        let traced = self.decoder.push_traced(bytes);
        self.absorb(traced)
    }

    /// Ends the stream, as when the connection closes. A receipt that was not cut is still returned
    /// if it printed something.
    pub fn finish(&mut self) -> Vec<Receipt> {
        self.finish_traced().receipts
    }

    /// As [`Printer::finish`], with the events the end of the stream made.
    pub fn finish_traced(&mut self) -> Pushed {
        let traced = self.decoder.finish_traced();
        let mut pushed = self.absorb(traced);
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
}
