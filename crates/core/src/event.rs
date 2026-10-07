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
    /// 0 is font A, 1 is font B, and up to 4 is font E.
    pub font: u8,
    /// Printed twice, a little offset, which looks like bold.
    pub double_strike: bool,
    /// The line is turned half a turn.
    pub upside_down: bool,
    /// The characters are turned a quarter turn clockwise.
    pub rotated: bool,
    /// 0 is the first colour, black, and 1 the second, usually red.
    pub color: u8,
    /// Edges of big characters are smoothed.
    pub smooth: bool,
    /// Space added to the right of each character, in horizontal motion units.
    pub spacing: u8,
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
            double_strike: false,
            upside_down: false,
            rotated: false,
            color: 0,
            smooth: false,
            spacing: 0,
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
    Gs1_128,
    Gs1DataBarOmnidirectional,
    Gs1DataBarTruncated,
    Gs1DataBarLimited,
    Gs1DataBarExpanded,
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
            Symbology::Gs1_128 => "gs1-128",
            Symbology::Gs1DataBarOmnidirectional => "gs1-databar-omnidirectional",
            Symbology::Gs1DataBarTruncated => "gs1-databar-truncated",
            Symbology::Gs1DataBarLimited => "gs1-databar-limited",
            Symbology::Gs1DataBarExpanded => "gs1-databar-expanded",
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
    /// What is encoded, as text. For Code 128 the character set selectors such as `{B` are left
    /// out, and the pairs of digits of set C are written out as digits.
    pub data: String,
    /// The bytes as they were sent after the length or before the end byte, selectors included.
    pub raw: Vec<u8>,
    /// Height of the bars in dots.
    pub height: u8,
    /// Width of the narrowest bar in dots.
    pub width: u8,
    pub text: TextPosition,
    /// The font of the readable text: 0 is font A and 1 is font B.
    pub font: u8,
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
    /// 1 or 2, or 3 for a Micro QR code.
    pub model: u8,
    /// Size of one module in dots.
    pub size: u8,
    pub level: QrLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdf417 {
    pub data: String,
    /// How many columns of data, or 0 to pick the number from the data.
    pub columns: u8,
    /// How many rows, or 0 to pick the number from the data.
    pub rows: u8,
    /// Width of one module in dots.
    pub width: u8,
    /// How many times higher than wide a module is.
    pub height: u8,
    /// 48 says `level` is a level, and 49 says it is a percentage of the data.
    pub level_mode: u8,
    pub level: u8,
    /// Without the stop bar and the right row indicator, which makes the code narrower.
    pub truncated: bool,
}

/// The two dimensional codes other than QR and PDF417.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    MaxiCode,
    Gs1DataBar,
    Composite,
    Aztec,
    DataMatrix,
}

impl SymbolKind {
    pub fn name(self) -> &'static str {
        match self {
            SymbolKind::MaxiCode => "maxicode",
            SymbolKind::Gs1DataBar => "gs1-databar-2d",
            SymbolKind::Composite => "composite",
            SymbolKind::Aztec => "aztec",
            SymbolKind::DataMatrix => "datamatrix",
        }
    }
}

/// A two dimensional code that is not QR or PDF417, with the settings sent before its data. The
/// settings are the raw bytes of each `GS ( k` function, by function number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub data: Vec<u8>,
    pub settings: Vec<(u8, Vec<u8>)>,
}

/// The cash drawer kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    /// Which drawer connector, 0 or 1.
    pub pin: u8,
    pub on_ms: u16,
    pub off_ms: u16,
}

/// A move of the print position along the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// From the left edge of the print area, in horizontal motion units.
    Absolute(u16),
    /// From where the print position is now, and it can be negative.
    Relative(i16),
}

/// How far apart the lines are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineSpacing {
    /// A sixth of an inch.
    Default,
    /// This many vertical motion units.
    Units(u8),
    /// This many sixtieths of an inch.
    Sixtieths(u8),
    /// This many three hundred and sixtieths of an inch.
    ThreeSixtieths(u8),
    /// This many dots, in the Star command sets.
    Dots(u16),
}

/// A character that was drawn by the program that sent it, with `ESC &`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCharacter {
    /// The character code, from 32 to 126, that prints it.
    pub code: u8,
    /// How many dots wide it is.
    pub width: u8,
    /// How many bytes tall each column is. Each byte is 8 dots.
    pub column_bytes: u8,
    /// The columns, left to right, the top dot in the highest bit of each byte.
    pub data: Vec<u8>,
}

impl UserCharacter {
    /// The character that stands for this one in the text of a `Text` event, in the private use
    /// area, so a drawer can tell it from the font's own character with the same code.
    pub fn glyph(code: u8) -> char {
        char::from_u32(0xe000 + u32::from(code)).unwrap_or('\u{fffd}')
    }
}

/// What page mode does. In page mode the printer lays out a page in a box and prints it at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// `ESC L`: page mode begins.
    Enter,
    /// `ESC S`, or `FF` after printing: standard mode again.
    Standard,
    /// `ESC T`: 0 left to right, 1 bottom to top, 2 right to left, 3 top to bottom.
    Direction(u8),
    /// `ESC W`: where the page is, in motion units.
    Area {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    /// `GS $` and `GS \`: a move in the other direction than the text runs.
    Vertical(Position),
    /// `ESC FF` and `FF`: print what was laid out.
    Print,
    /// `CAN`: throw away what was laid out in the area.
    Cancel,
}

/// One thing a printer was asked to do, in the order the bytes asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The printer was reset to its power on settings.
    Initialize,
    Align(Align),
    /// The font was chosen: 0 is font A, 1 is font B, and up to 4 is font E.
    Font(u8),
    /// The paper was fed by this many lines.
    Feed(u8),
    /// The paper was fed by this many vertical motion units, or dots in the Star command sets.
    FeedUnits(u16),
    /// The paper was fed backwards by this many lines.
    ReverseFeed(u8),
    LineSpacing(LineSpacing),
    /// The print position moved along the line, so what follows starts there.
    Position(Position),
    /// The left margin, in horizontal motion units.
    Margin(u16),
    /// The width of the print area, in horizontal motion units.
    PrintWidth(u16),
    /// How big a motion unit is: a horizontal one is 1/x inch and a vertical one is 1/y inch. 0
    /// means the printer's own default.
    MotionUnits {
        x: u8,
        y: u8,
    },
    /// The columns the tab character jumps to. Empty means no tab stops.
    TabStops(Vec<u8>),
    /// How dark the print is, in tenths of a percent from the usual, from -500 to 500.
    Density(i16),
    /// A line, or the part of a line printed in one style. A blank line has empty text.
    Text {
        text: String,
        style: Style,
        align: Align,
    },
    /// A picture, or a band of one. Bands of one picture arrive one after the other. The dots are
    /// printer dots, so a picture that prints at half the density is already doubled.
    Image(Bitmap),
    Barcode(Barcode),
    Qr(Qr),
    Pdf417(Pdf417),
    Symbol(Symbol),
    Cut(CutMode),
    Pulse(Pulse),
    /// The buzzer sounded this many times. The duration is in units of 100 ms, or 0 when the
    /// command does not say.
    Beep {
        times: u8,
        duration: u8,
    },
    /// `ESC &`: a character was drawn.
    DefineCharacter(UserCharacter),
    /// `ESC ?`: the drawn character with this code is gone.
    DeleteCharacter(u8),
    /// `ESC %`: drawn characters are used, or not.
    UserCharacters(bool),
    Page(Page),
    /// Bytes that are not a command this crate knows, where they started in the stream.
    Unknown {
        offset: usize,
        bytes: Vec<u8>,
    },
}
