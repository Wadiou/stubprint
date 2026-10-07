//! Decodes the bytes a receipt printer receives into a document that can be drawn.
//!
//! The crate has no networking and no user interface. A [`Decoder`] reads bytes, in any number of
//! pieces, and produces the [`Event`]s a printer would act on: text, pictures, barcodes, QR codes,
//! feeds and cuts. A [`Printer`] groups those events into [`Receipt`]s.

mod bitmap;
mod codepage;
mod decoder;
mod event;
mod printer;
mod profile;
mod status;

pub use bitmap::Bitmap;
pub use decoder::Decoder;
pub use event::{
    Align, Barcode, CutMode, Event, LineSpacing, Page, Pdf417, Position, Pulse, Qr, QrLevel, Style,
    Symbol, SymbolKind, Symbology, TextPosition, UserCharacter,
};
pub use printer::{Printer, Receipt};
pub use profile::{Identity, KanjiEncoding, Profile};
pub use status::{Fault, Paper, PrinterStatus};
