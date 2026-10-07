use crate::bitmap::Bitmap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

impl Align {
    pub fn name(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    Full,
    Partial,
}

impl CutMode {
    pub fn name(self) -> &'static str {
        match self {
            CutMode::Full => "full",
            CutMode::Partial => "partial",
        }
    }
}

/// How text is drawn, as the printer was set when the text arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    /// 0 is off, 1 is a thin line and 2 is a thick one.
    pub underline: u8,
    pub invert: bool,
    /// How many times wider than normal, 1 to 8.
    pub width: u8,
    /// How many times taller than normal, 1 to 8.
    pub height: u8,
    /// 0 is font A and 1 is font B.
    pub font: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bold: false,
            underline: 0,
            invert: false,
            width: 1,
            height: 1,
            font: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbology {
    UpcA,
    UpcE,
    Ean13,
    Ean8,
    Code39,
    Itf,
    Codabar,
    Code93,
    Code128,
}

impl Symbology {
    pub fn name(self) -> &'static str {
        match self {
            Symbology::UpcA => "upca",
            Symbology::UpcE => "upce",
            Symbology::Ean13 => "ean13",
            Symbology::Ean8 => "ean8",
            Symbology::Code39 => "code39",
            Symbology::Itf => "itf",
            Symbology::Codabar => "codabar",
            Symbology::Code93 => "code93",
            Symbology::Code128 => "code128",
        }
    }
}

/// Where the readable text of a barcode goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPosition {
    None,
    Above,
    Below,
    Both,
}

impl TextPosition {
    pub fn name(self) -> &'static str {
        match self {
            TextPosition::None => "none",
            TextPosition::Above => "above",
            TextPosition::Below => "below",
            TextPosition::Both => "both",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barcode {
    pub symbology: Symbology,
    /// What is encoded. For Code 128 the character set selector, such as `{B`, is left out.
    pub data: String,
    /// Height of the bars in dots.
    pub height: u8,
    /// Width of the narrowest bar in dots.
    pub width: u8,
    pub text: TextPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrLevel {
    L,
    M,
    Q,
    H,
}

impl QrLevel {
    pub fn name(self) -> &'static str {
        match self {
            QrLevel::L => "L",
            QrLevel::M => "M",
            QrLevel::Q => "Q",
            QrLevel::H => "H",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qr {
    pub data: String,
    /// 1 or 2.
    pub model: u8,
    /// Size of one module in dots.
    pub size: u8,
    pub level: QrLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdf417 {
    pub data: String,
}

/// The cash drawer kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    /// Which drawer connector, 0 or 1.
    pub pin: u8,
    pub on_ms: u16,
    pub off_ms: u16,
}

/// One thing a printer was asked to do, in the order the bytes asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The printer was reset to its power on settings.
    Initialize,
    Align(Align),
    /// The font was chosen: 0 is font A and 1 is font B.
    Font(u8),
    /// The paper was fed by this many lines.
    Feed(u8),
    /// A line, or the part of a line printed in one style. A blank line has empty text.
    Text {
        text: String,
        style: Style,
        align: Align,
    },
    /// A band of a picture. Bands of one picture arrive one after the other.
    Image(Bitmap),
    Barcode(Barcode),
    Qr(Qr),
    Pdf417(Pdf417),
    Cut(CutMode),
    Pulse(Pulse),
    /// Bytes that are not a command this crate knows, where they started in the stream.
    Unknown {
        offset: usize,
        bytes: Vec<u8>,
    },
}
