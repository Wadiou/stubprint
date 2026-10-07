use serde::{Serialize, Serializer};

use crate::bitmap::Bitmap;

/// Writes an enum with a `name` method as that name, so the names of the JSON are the ones of the
/// configuration and the documents, written once.
macro_rules! serialize_as_name {
    ($($type:ty),*) => {$(
        impl Serialize for $type {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.name())
            }
        }
    )*};
}

/// Bytes are written as hexadecimal text, which is easy to read and to compare.
pub(crate) mod hex {
    use serde::Serializer;

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        let text: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        serializer.serialize_str(&text)
    }

    pub fn serialize_pairs<S: Serializer>(
        pairs: &[(u8, Vec<u8>)],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        struct Hex<'a>(&'a [u8]);
        impl serde::Serialize for Hex<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serialize(self.0, serializer)
            }
        }
        let mut seq = serializer.serialize_seq(Some(pairs.len()))?;
        for (function, bytes) in pairs {
            seq.serialize_element(&(function, Hex(bytes)))?;
        }
        seq.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(rename_all = "lowercase"))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(rename_all = "lowercase"))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Style {
    pub bold: bool,
    /// 0 is off, 1 is a thin line and 2 is a thick one.
    pub underline: u8,
    /// A line over the characters, which only the Star languages have.
    pub overline: bool,
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
            overline: false,
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Symbology {
    #[cfg_attr(feature = "openapi", schema(rename = "upca"))]
    UpcA,
    #[cfg_attr(feature = "openapi", schema(rename = "upce"))]
    UpcE,
    #[cfg_attr(feature = "openapi", schema(rename = "ean13"))]
    Ean13,
    #[cfg_attr(feature = "openapi", schema(rename = "ean8"))]
    Ean8,
    #[cfg_attr(feature = "openapi", schema(rename = "code39"))]
    Code39,
    #[cfg_attr(feature = "openapi", schema(rename = "itf"))]
    Itf,
    #[cfg_attr(feature = "openapi", schema(rename = "codabar"))]
    Codabar,
    #[cfg_attr(feature = "openapi", schema(rename = "code93"))]
    Code93,
    #[cfg_attr(feature = "openapi", schema(rename = "code128"))]
    Code128,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-128"))]
    Gs1_128,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-databar-omnidirectional"))]
    Gs1DataBarOmnidirectional,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-databar-truncated"))]
    Gs1DataBarTruncated,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-databar-limited"))]
    Gs1DataBarLimited,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-databar-expanded"))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(rename_all = "lowercase"))]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Barcode {
    pub symbology: Symbology,
    /// What is encoded, as text. For Code 128 the character set selectors such as `{B` are left
    /// out, and the pairs of digits of set C are written out as digits.
    pub data: String,
    /// The bytes as they were sent after the length or before the end byte, selectors included.
    #[serde(serialize_with = "hex::serialize")]
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(rename_all = "UPPERCASE"))]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Qr {
    pub data: String,
    /// 1 or 2, or 3 for a Micro QR code.
    pub model: u8,
    /// Size of one module in dots.
    pub size: u8,
    pub level: QrLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum SymbolKind {
    #[cfg_attr(feature = "openapi", schema(rename = "maxicode"))]
    MaxiCode,
    #[cfg_attr(feature = "openapi", schema(rename = "gs1-databar-2d"))]
    Gs1DataBar,
    #[cfg_attr(feature = "openapi", schema(rename = "composite"))]
    Composite,
    #[cfg_attr(feature = "openapi", schema(rename = "aztec"))]
    Aztec,
    #[cfg_attr(feature = "openapi", schema(rename = "datamatrix"))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Symbol {
    pub kind: SymbolKind,
    #[serde(serialize_with = "hex::serialize")]
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub data: Vec<u8>,
    #[serde(serialize_with = "hex::serialize_pairs")]
    #[cfg_attr(feature = "openapi", schema(value_type = Vec<(u8, String)>))]
    pub settings: Vec<(u8, Vec<u8>)>,
}

serialize_as_name!(Align, CutMode, Symbology, TextPosition, QrLevel, SymbolKind);

/// The cash drawer kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Pulse {
    /// Which drawer connector, 0 or 1.
    pub pin: u8,
    pub on_ms: u16,
    pub off_ms: u16,
}

/// A move of the print position along the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// From the left edge of the print area, in horizontal motion units.
    Absolute(u16),
    /// From where the print position is now, and it can be negative.
    Relative(i16),
}

/// How far apart the lines are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UserCharacter {
    /// The character code, from 32 to 126, that prints it.
    pub code: u8,
    /// How many dots wide it is.
    pub width: u8,
    /// How many bytes tall each column is. Each byte is 8 dots.
    pub column_bytes: u8,
    /// The columns, left to right, the top dot in the highest bit of each byte.
    #[serde(serialize_with = "hex::serialize")]
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub data: Vec<u8>,
}

impl UserCharacter {
    /// The character that stands for this one in the text of a `Text` event, in the private use
    /// area, so a drawer can tell it from the font's own character with the same code.
    pub fn glyph(code: u8) -> char {
        char::from_u32(0xe000 + u32::from(code)).unwrap_or('\u{fffd}')
    }
}

/// A Kanji character that was drawn by the program that sent it, with `FS 2`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UserKanji {
    /// The first and the second byte of the code that prints it, as they were sent.
    pub code: [u8; 2],
    /// How many dots wide it is: 24, 20 or 16.
    pub width: u8,
    /// How many dots tall it is: 24 or 16.
    pub height: u8,
    /// The columns, left to right, each `height / 8` bytes with the top dot in the highest bit.
    #[serde(serialize_with = "hex::serialize")]
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub data: Vec<u8>,
}

impl UserKanji {
    /// The character that stands for the drawn Kanji with this code in the text of a `Text` event,
    /// in the private use area after the ones of [`UserCharacter::glyph`]. The codes a printer lets
    /// a program draw are 94 in a row, whichever code system is in use: 0x21 to 0x7e after 0x77 in
    /// JIS, 0x40 to 0x7e and 0x80 to 0x9e after 0xec in Shift JIS, and 0xa1 to 0xfe after 0xfe for
    /// the Chinese and Korean models. Codes outside those have no glyph.
    pub fn glyph(code: [u8; 2]) -> Option<char> {
        let index = match code {
            [0x77, second @ 0x21..=0x7e] => second - 0x21,
            [0xec, second @ 0x40..=0x7e] => second - 0x40,
            [0xec, second @ 0x80..=0x9e] => second - 0x80 + 63,
            [0xfe, second @ 0xa1..=0xfe] => second - 0xa1,
            _ => return None,
        };
        char::from_u32(0xe100 + u32::from(index))
    }
}

/// What page mode does. In page mode the printer lays out a page in a box and prints it at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
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

/// How far the paper was fed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FeedAmount {
    Lines(u8),
    /// Vertical motion units, or dots in the Star command sets.
    Units(u16),
    /// To the top of the next page, which a Star form feed asks for.
    Page,
}

/// The paper was moved, after what was printed before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Feed {
    pub amount: FeedAmount,
    /// The paper went backwards.
    pub reverse: bool,
}

/// One thing a printer was asked to do, in the order the bytes asked for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Event {
    /// The printer was reset to its power on settings.
    Initialize,
    Align(Align),
    /// The font was chosen: 0 is font A, 1 is font B, and up to 4 is font E.
    Font(u8),
    Feed(Feed),
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
    /// A line feed ended the line: the text before it, since the last line feed, is one printed line,
    /// and what comes next starts a new one. Text that something other than a line feed ended, such
    /// as a feed or a picture, has no such event.
    LineFeed,
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
    /// `FS 2`: a Kanji character was drawn.
    DefineKanji(UserKanji),
    /// `GS ( L` function 49: the graphics that follow are made for this many dots per inch, 180 or
    /// 360, in both directions.
    GraphicsDensity(u16),
    Page(Page),
    /// Bytes that are not a command this crate knows, where they started in the stream.
    Unknown {
        offset: usize,
        #[serde(serialize_with = "hex::serialize")]
        #[cfg_attr(feature = "openapi", schema(value_type = String))]
        bytes: Vec<u8>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn written(event: &Event) -> serde_json::Value {
        serde_json::to_value(event).unwrap()
    }

    #[test]
    fn an_event_is_written_with_its_type_and_its_value() {
        assert_eq!(written(&Event::Initialize), json!({"type": "initialize"}));
        assert_eq!(
            written(&Event::Cut(CutMode::Partial)),
            json!({"type": "cut", "value": "partial"})
        );
        assert_eq!(
            written(&Event::Position(Position::Relative(-3))),
            json!({"type": "position", "value": {"relative": -3}})
        );
        assert_eq!(
            written(&Event::Feed(Feed {
                amount: FeedAmount::Page,
                reverse: false
            })),
            json!({"type": "feed", "value": {"amount": "page", "reverse": false}})
        );
    }

    #[test]
    fn names_are_the_ones_the_configuration_uses() {
        let barcode = Event::Barcode(Barcode {
            symbology: Symbology::Gs1_128,
            data: "1".into(),
            raw: vec![0x7b, 0x42, 0x31],
            height: 80,
            width: 3,
            text: TextPosition::Below,
            font: 0,
        });
        let value = written(&barcode);
        assert_eq!(value["value"]["symbology"], "gs1-128");
        assert_eq!(value["value"]["text"], "below");
        assert_eq!(value["value"]["raw"], "7b4231");
    }

    #[test]
    fn a_picture_is_written_as_its_size_and_its_rows_in_base64() {
        let bitmap = Bitmap::from_packed(1, 2, vec![0xff, 0x00]).unwrap();
        assert_eq!(
            written(&Event::Image(bitmap)),
            json!({"type": "image", "value": {"width": 8, "height": 2, "row_bytes": 1, "data": "/wA="}})
        );
    }

    #[test]
    fn bytes_nobody_understood_are_written_in_hex_with_their_place() {
        assert_eq!(
            written(&Event::Unknown {
                offset: 4,
                bytes: vec![0x1b, 0x01]
            }),
            json!({"type": "unknown", "value": {"offset": 4, "bytes": "1b01"}})
        );
    }
}
