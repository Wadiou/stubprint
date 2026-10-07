//! Decodes the bytes a receipt printer receives into a document that can be drawn.
//!
//! The crate has no networking and no user interface. A decoder reads bytes, in any number of
//! pieces, and produces the items a printer would print: text, images, barcodes, QR codes,
//! feeds and cuts.
