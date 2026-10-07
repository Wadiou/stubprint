use crate::bitmap::Bitmap;
use crate::decoder::{Decoder, Traced};
use crate::dialect::Dialect;
use crate::event::Event;
use crate::profile::Profile;
use crate::status::PrinterStatus;

/// Everything a printer printed between the start of a job and its cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    events: Vec<Event>,
    raw: Vec<u8>,
}

impl Receipt {
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The bytes the printer was sent for this receipt, up to the end of the cut.
    pub fn raw(&self) -> &[u8] {
        &self.raw
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
        self.stream.extend_from_slice(bytes);
        let events = self.decoder.push_traced(bytes);
        self.absorb(events)
    }

    /// Ends the stream, as when the connection closes. A receipt that was not cut is still returned
    /// if it printed something.
    pub fn finish(&mut self) -> Vec<Receipt> {
        let events = self.decoder.finish_traced();
        let mut receipts = self.absorb(events);
        let rest = std::mem::take(&mut self.events);
        let raw = std::mem::take(&mut self.stream);
        self.before += raw.len();
        if prints_something(&rest) {
            receipts.push(Receipt { events: rest, raw });
        }
        receipts
    }

    fn absorb(&mut self, events: Vec<Traced>) -> Vec<Receipt> {
        let mut receipts = Vec::new();
        for Traced { event, end } in events {
            let is_cut = matches!(event, Event::Cut(_));
            self.events.push(event);
            if is_cut {
                let events = std::mem::take(&mut self.events);
                if prints_something(&events) {
                    let raw: Vec<u8> = self.stream.drain(..end - self.before).collect();
                    self.before = end;
                    receipts.push(Receipt { events, raw });
                }
            }
        }
        receipts
    }
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
}
