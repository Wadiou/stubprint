use crate::bitmap::Bitmap;
use crate::codepage;
use crate::event::{
    Align, Barcode, CutMode, Event, Pdf417, Pulse, Qr, QrLevel, Style, Symbology, TextPosition,
};

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const FS: u8 = 0x1c;
const LF: u8 = 0x0a;
const CR: u8 = 0x0d;
const FIRST_TEXT_BYTE: u8 = 0x20;

/// `GS ( k` selects a symbol with this first byte: 48 is PDF417 and 49 is QR.
const PDF417_CODE: u8 = 0x30;
const QR_CODE: u8 = 0x31;

/// Not enough bytes yet to know what the command is, or to read all of it.
enum Step {
    Consumed(usize),
    Incomplete,
}

/// What the printer was last told to do, which decides how the next bytes are read.
#[derive(Debug, Clone)]
struct State {
    style: Style,
    align: Align,
    codepage: u8,
    barcode_height: u8,
    barcode_width: u8,
    barcode_text: TextPosition,
    qr_model: u8,
    qr_size: u8,
    qr_level: QrLevel,
    qr_data: Vec<u8>,
    pdf417_data: Vec<u8>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            style: Style::default(),
            align: Align::Left,
            codepage: 0,
            barcode_height: 162,
            barcode_width: 3,
            barcode_text: TextPosition::None,
            qr_model: 2,
            qr_size: 3,
            qr_level: QrLevel::L,
            qr_data: Vec::new(),
            pdf417_data: Vec::new(),
        }
    }
}

/// Reads the bytes a printer would receive and says what they ask for. It accepts the bytes in any
/// number of pieces, since a network or Bluetooth link can cut a command anywhere, and holds on to
/// a half received command until the rest arrives. The same bytes give the same events however
/// they are split.
#[derive(Debug, Default)]
pub struct Decoder {
    pending: Vec<u8>,
    /// How many bytes were consumed before the ones in `pending`.
    consumed: usize,
    state: State,
    /// The bytes of the line being printed, read in the code page when the line ends.
    line: Vec<u8>,
    /// Whether the current line has already been reported, so a line feed does not add a blank one.
    line_reported: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds received bytes and returns the events they completed.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Event> {
        self.run(chunk, false)
    }

    /// Ends the stream. A line with no line feed is printed, and bytes that never became a whole
    /// command are reported as unknown.
    pub fn finish(&mut self) -> Vec<Event> {
        let mut events = self.run(&[], true);
        self.flush_line(&mut events);
        events
    }

    /// How many received bytes are waiting for the rest of their command.
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    fn run(&mut self, chunk: &[u8], at_end: bool) -> Vec<Event> {
        let mut bytes = std::mem::take(&mut self.pending);
        bytes.extend_from_slice(chunk);

        let mut events = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            match self.parse(
                &bytes[position..],
                self.consumed + position,
                at_end,
                &mut events,
            ) {
                Step::Consumed(length) => position += length,
                Step::Incomplete => break,
            }
        }

        if at_end && position < bytes.len() {
            events.push(Event::Unknown {
                offset: self.consumed + position,
                bytes: bytes[position..].to_vec(),
            });
            position = bytes.len();
        }
        self.consumed += position;
        self.pending = bytes.split_off(position);
        events
    }

    fn parse(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        match bytes[0] {
            ESC => self.parse_esc(bytes, offset, at_end, events),
            GS => self.parse_gs(bytes, offset, at_end, events),
            FS => match bytes.get(1) {
                None => Step::Incomplete,
                // FS . cancels the Kanji mode some printers start in.
                Some(0x2e) => Step::Consumed(2),
                Some(_) => unknown_run(bytes, offset, at_end, events),
            },
            LF => {
                self.line_feed(events);
                Step::Consumed(1)
            }
            CR => Step::Consumed(1),
            byte if byte >= FIRST_TEXT_BYTE => {
                let length = bytes
                    .iter()
                    .position(|byte| *byte < FIRST_TEXT_BYTE)
                    .unwrap_or(bytes.len());
                self.line.extend_from_slice(&bytes[..length]);
                Step::Consumed(length)
            }
            _ => unknown_run(bytes, offset, at_end, events),
        }
    }

    fn parse_esc(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(&code) = bytes.get(1) else {
            return Step::Incomplete;
        };
        match code {
            // ESC @ resets the printer.
            0x40 => {
                self.flush_line(events);
                self.state = State::default();
                events.push(Event::Initialize);
                Step::Consumed(2)
            }
            // ESC 2 selects the default line spacing.
            0x32 => Step::Consumed(2),
            // ESC a n, ESC d n, ESC 3 n, ESC E n, ESC - n, ESC M n and ESC t n take one byte.
            0x61 | 0x64 | 0x33 | 0x45 | 0x2d | 0x4d | 0x74 => {
                let Some(&value) = bytes.get(2) else {
                    return Step::Incomplete;
                };
                if code == 0x61 && value > 2 && !(48..=50).contains(&value) {
                    return unknown_run(bytes, offset, at_end, events);
                }
                self.one_byte_command(code, value, events);
                Step::Consumed(3)
            }
            // ESC p m t1 t2 kicks the cash drawer.
            0x70 => {
                if bytes.len() < 5 {
                    return Step::Incomplete;
                }
                self.flush_line(events);
                events.push(Event::Pulse(Pulse {
                    pin: bytes[2] & 1,
                    on_ms: u16::from(bytes[3]) * 2,
                    off_ms: u16::from(bytes[4]) * 2,
                }));
                Step::Consumed(5)
            }
            // ESC * m nL nH d... is a picture in columns.
            0x2a => self.parse_column_image(bytes, offset, at_end, events),
            _ => unknown_run(bytes, offset, at_end, events),
        }
    }

    fn one_byte_command(&mut self, code: u8, value: u8, events: &mut Vec<Event>) {
        match code {
            0x61 => {
                self.flush_line(events);
                self.state.align = match value & 3 {
                    1 => Align::Center,
                    2 => Align::Right,
                    _ => Align::Left,
                };
                events.push(Event::Align(self.state.align));
            }
            0x64 => {
                self.flush_line(events);
                events.push(Event::Feed(value));
            }
            // ESC 3 n sets the line spacing, which changes nothing that is drawn here.
            0x33 => {}
            0x45 => self.change_style(events, |style| style.bold = value & 1 == 1),
            0x2d => self.change_style(events, |style| style.underline = value & 3),
            0x4d => {
                let font = value & 1;
                self.change_style(events, |style| style.font = font);
                events.push(Event::Font(font));
            }
            _ => {
                self.flush_line(events);
                self.state.codepage = value;
            }
        }
    }

    fn parse_column_image(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        if bytes.len() < 5 {
            return Step::Incomplete;
        }
        // Modes 0 and 1 have 8 dots in a column, modes 32 and 33 have 24.
        let bytes_per_column = match bytes[2] {
            0 | 1 => 1,
            32 | 33 => 3,
            _ => return unknown_run(bytes, offset, at_end, events),
        };
        let columns = usize::from(bytes[3]) + usize::from(bytes[4]) * 256;
        let length = 5 + columns * bytes_per_column;
        if bytes.len() < length {
            return Step::Incomplete;
        }
        self.flush_line(events);
        if columns > 0 {
            let mut bitmap = Bitmap::new(columns as u32, (bytes_per_column * 8) as u32);
            for (column, data) in bytes[5..length].chunks(bytes_per_column).enumerate() {
                for (index, byte) in data.iter().enumerate() {
                    for bit in 0..8 {
                        if byte & (0x80 >> bit) != 0 {
                            bitmap.set_black(column as u32, (index * 8 + bit) as u32);
                        }
                    }
                }
            }
            events.push(Event::Image(bitmap));
        }
        Step::Consumed(length)
    }

    fn parse_gs(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(&code), Some(&third)) = (bytes.get(1), bytes.get(2)) else {
            return Step::Incomplete;
        };
        match code {
            // GS V m cuts the paper.
            0x56 => {
                let mode = match third {
                    0 | 48 => CutMode::Full,
                    1 | 49 => CutMode::Partial,
                    _ => return unknown_run(bytes, offset, at_end, events),
                };
                self.flush_line(events);
                events.push(Event::Cut(mode));
                Step::Consumed(3)
            }
            // GS h n, GS w n and GS H n set up the next barcode. GS f n picks the font for its text.
            0x68 => {
                self.state.barcode_height = third;
                Step::Consumed(3)
            }
            0x77 => {
                self.state.barcode_width = third;
                Step::Consumed(3)
            }
            0x48 => {
                self.state.barcode_text = match third & 3 {
                    1 => TextPosition::Above,
                    2 => TextPosition::Below,
                    3 => TextPosition::Both,
                    _ => TextPosition::None,
                };
                Step::Consumed(3)
            }
            0x66 => Step::Consumed(3),
            // GS ! n sets the character size and GS B n turns white on black on or off.
            0x21 => {
                let (width, height) = ((third >> 4) + 1, (third & 0x0f) + 1);
                self.change_style(events, |style| {
                    style.width = width;
                    style.height = height;
                });
                Step::Consumed(3)
            }
            0x42 => {
                self.change_style(events, |style| style.invert = third & 1 == 1);
                Step::Consumed(3)
            }
            0x6b => self.parse_barcode(bytes, offset, at_end, events),
            0x28 if third == 0x6b => self.parse_symbol(bytes, offset, at_end, events),
            0x76 if third == 0x30 => self.parse_raster(bytes, events),
            _ => unknown_run(bytes, offset, at_end, events),
        }
    }

    /// `GS k m d... NUL` for the older forms and `GS k m n d...` for the ones with a length.
    fn parse_barcode(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        let mode = bytes[2];
        let (symbology, data_start, data_end, length) = if mode <= 6 {
            let Some(end) = bytes[3..].iter().position(|byte| *byte == 0) else {
                return Step::Incomplete;
            };
            let symbology = [
                Symbology::UpcA,
                Symbology::UpcE,
                Symbology::Ean13,
                Symbology::Ean8,
                Symbology::Code39,
                Symbology::Itf,
                Symbology::Codabar,
            ][usize::from(mode)];
            (symbology, 3, 3 + end, 3 + end + 1)
        } else if (65..=73).contains(&mode) {
            let Some(&count) = bytes.get(3) else {
                return Step::Incomplete;
            };
            let length = 4 + usize::from(count);
            if bytes.len() < length {
                return Step::Incomplete;
            }
            let symbology = [
                Symbology::UpcA,
                Symbology::UpcE,
                Symbology::Ean13,
                Symbology::Ean8,
                Symbology::Code39,
                Symbology::Itf,
                Symbology::Codabar,
                Symbology::Code93,
                Symbology::Code128,
            ][usize::from(mode - 65)];
            (symbology, 4, length, length)
        } else {
            return unknown_run(bytes, offset, at_end, events);
        };

        let mut data = &bytes[data_start..data_end];
        if symbology == Symbology::Code128 && data.len() >= 2 && data[0] == b'{' {
            data = &data[2..];
        }
        self.flush_line(events);
        events.push(Event::Barcode(Barcode {
            symbology,
            data: String::from_utf8_lossy(data).into_owned(),
            height: self.state.barcode_height,
            width: self.state.barcode_width,
            text: self.state.barcode_text,
        }));
        Step::Consumed(length)
    }

    /// `GS ( k pL pH cn fn ...` stores the settings and the data of a QR or PDF417 code, then prints it.
    fn parse_symbol(
        &mut self,
        bytes: &[u8],
        offset: usize,
        at_end: bool,
        events: &mut Vec<Event>,
    ) -> Step {
        if bytes.len() < 5 {
            return Step::Incomplete;
        }
        let length = 5 + usize::from(bytes[3]) + usize::from(bytes[4]) * 256;
        if bytes.len() < length {
            return Step::Incomplete;
        }
        let (Some(&symbol), Some(&function)) = (bytes.get(5), bytes.get(6)) else {
            return unknown_run(bytes, offset, at_end, events);
        };
        let argument = bytes.get(7).copied().unwrap_or(0);
        let data = bytes.get(8..length).unwrap_or(&[]);

        match (symbol, function) {
            (QR_CODE, 0x41) => self.state.qr_model = if argument == 49 { 1 } else { 2 },
            (QR_CODE, 0x43) => self.state.qr_size = argument,
            (QR_CODE, 0x45) => {
                self.state.qr_level = match argument {
                    49 => QrLevel::M,
                    50 => QrLevel::Q,
                    51 => QrLevel::H,
                    _ => QrLevel::L,
                }
            }
            (QR_CODE, 0x50) => self.state.qr_data = data.to_vec(),
            (QR_CODE, 0x51) => {
                self.flush_line(events);
                events.push(Event::Qr(Qr {
                    data: String::from_utf8_lossy(&self.state.qr_data).into_owned(),
                    model: self.state.qr_model,
                    size: self.state.qr_size,
                    level: self.state.qr_level,
                }));
            }
            (PDF417_CODE, 0x50) => self.state.pdf417_data = data.to_vec(),
            (PDF417_CODE, 0x51) => {
                self.flush_line(events);
                events.push(Event::Pdf417(Pdf417 {
                    data: String::from_utf8_lossy(&self.state.pdf417_data).into_owned(),
                }));
            }
            // Columns, rows, size and error level of a PDF417 code change nothing that is drawn here.
            (PDF417_CODE, 0x41..=0x46) => {}
            _ => events.push(Event::Unknown {
                offset,
                bytes: bytes[..length].to_vec(),
            }),
        }
        Step::Consumed(length)
    }

    /// `GS v 0 m xL xH yL yH d...` is a picture in rows.
    fn parse_raster(&mut self, bytes: &[u8], events: &mut Vec<Event>) -> Step {
        if bytes.len() < 8 {
            return Step::Incomplete;
        }
        let row_bytes = usize::from(bytes[4]) + usize::from(bytes[5]) * 256;
        let height = usize::from(bytes[6]) + usize::from(bytes[7]) * 256;
        let length = 8 + row_bytes * height;
        if bytes.len() < length {
            return Step::Incomplete;
        }
        self.flush_line(events);
        if let Some(bitmap) =
            Bitmap::from_packed(row_bytes, height as u32, bytes[8..length].to_vec())
        {
            events.push(Event::Image(bitmap));
        }
        Step::Consumed(length)
    }

    /// Reports the text printed so far in the old style, then applies the change.
    fn change_style(&mut self, events: &mut Vec<Event>, change: impl FnOnce(&mut Style)) {
        self.flush_line(events);
        change(&mut self.state.style);
    }

    fn flush_line(&mut self, events: &mut Vec<Event>) {
        if self.line.is_empty() {
            return;
        }
        events.push(Event::Text {
            text: codepage::decode(self.state.codepage, &self.line),
            style: self.state.style.clone(),
            align: self.state.align,
        });
        self.line.clear();
        self.line_reported = true;
    }

    fn line_feed(&mut self, events: &mut Vec<Event>) {
        self.flush_line(events);
        if !self.line_reported {
            events.push(Event::Text {
                text: String::new(),
                style: self.state.style.clone(),
                align: self.state.align,
            });
        }
        self.line_reported = false;
    }
}

/// An unknown command is reported once, with the control bytes that follow it. A real printer
/// ignores those too. The run is kept open until a byte that ends it arrives, so the result does
/// not depend on where the stream was cut.
fn unknown_run(bytes: &[u8], offset: usize, at_end: bool, events: &mut Vec<Event>) -> Step {
    let mut end = if matches!(bytes[0], ESC | GS | FS) {
        2
    } else {
        1
    };
    if bytes.len() < end {
        return Step::Incomplete;
    }
    while end < bytes.len() && is_ignorable(bytes[end]) {
        end += 1;
    }
    if end == bytes.len() && !at_end {
        return Step::Incomplete;
    }
    events.push(Event::Unknown {
        offset,
        bytes: bytes[..end].to_vec(),
    });
    Step::Consumed(end)
}

fn is_ignorable(byte: u8) -> bool {
    byte < FIRST_TEXT_BYTE && !matches!(byte, LF | CR | ESC | FS | GS)
}
