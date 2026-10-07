use crate::bitmap::Bitmap;
use crate::decoder::Decoder;
use crate::event::Event;

/// Everything a printer printed between the start of a job and its cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    events: Vec<Event>,
}

impl Receipt {
    pub fn events(&self) -> &[Event] {
        &self.events
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
        Event::Image(_) | Event::Barcode(_) | Event::Qr(_) | Event::Pdf417(_) => true,
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
}

impl Printer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds received bytes and returns the receipts they finished.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Receipt> {
        let events = self.decoder.push(bytes);
        self.absorb(events)
    }

    /// Ends the stream, as when the connection closes. A receipt that was not cut is still returned
    /// if it printed something.
    pub fn finish(&mut self) -> Vec<Receipt> {
        let events = self.decoder.finish();
        let mut receipts = self.absorb(events);
        let rest = std::mem::take(&mut self.events);
        if prints_something(&rest) {
            receipts.push(Receipt { events: rest });
        }
        receipts
    }

    fn absorb(&mut self, events: Vec<Event>) -> Vec<Receipt> {
        let mut receipts = Vec::new();
        for event in events {
            let is_cut = matches!(event, Event::Cut(_));
            self.events.push(event);
            if is_cut {
                let events = std::mem::take(&mut self.events);
                if prints_something(&events) {
                    receipts.push(Receipt { events });
                }
            }
        }
        receipts
    }
}
