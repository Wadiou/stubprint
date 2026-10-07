//! Barcodes and two dimensional codes.

use super::super::machine::Machine;
use super::super::reader::{Input, Step, MAX_BARCODE_BYTES};
use super::{Star, RS};
use crate::event::{Barcode, Event, Pdf417, Qr, QrLevel, Symbology, TextPosition};

/// What `ESC GS y S` set for a QR code, and the data that `ESC GS y D` gave.
#[derive(Debug, Clone)]
pub(super) struct QrSettings {
    model: u8,
    level: QrLevel,
    cell: u8,
    data: Vec<u8>,
}

impl Default for QrSettings {
    fn default() -> Self {
        Self {
            model: 2,
            level: QrLevel::L,
            cell: 3,
            data: Vec::new(),
        }
    }
}

/// What `ESC GS x S` set for a PDF417 code, and the data that `ESC GS x D` gave.
#[derive(Debug, Clone)]
pub(super) struct Pdf417Settings {
    /// 0 gives the proportion of height to width, and 1 the rows and columns.
    size_mode: u8,
    first: u8,
    second: u8,
    level: u8,
    module_width: u8,
    module_ratio: u8,
    data: Vec<u8>,
}

impl Default for Pdf417Settings {
    fn default() -> Self {
        Self {
            size_mode: 0,
            first: 1,
            second: 2,
            level: 1,
            module_width: 2,
            module_ratio: 3,
            data: Vec::new(),
        }
    }
}

/// The barcode that a number of `ESC b` is, and whether it is one of the older ones that have
/// only 0 to 8.
fn symbology(n: u8) -> Option<Symbology> {
    Some(match n {
        0 | b'0' => Symbology::UpcE,
        1 | b'1' => Symbology::UpcA,
        2 | b'2' => Symbology::Ean8,
        3 | b'3' => Symbology::Ean13,
        4 | b'4' => Symbology::Code39,
        5 | b'5' => Symbology::Itf,
        6 | b'6' => Symbology::Code128,
        7 | b'7' => Symbology::Code93,
        8 | b'8' => Symbology::Codabar,
        9 | b'9' => Symbology::Gs1_128,
        10 | b'A' => Symbology::Gs1DataBarOmnidirectional,
        11 | b'B' => Symbology::Gs1DataBarTruncated,
        12 | b'C' => Symbology::Gs1DataBarLimited,
        13 | b'D' => Symbology::Gs1DataBarExpanded,
        _ => return None,
    })
}

/// Where the text goes, which font it is in, and whether the paper is fed after the barcode, for
/// a number of `ESC b`. The digits 1 to 9 and the letters A to E are the numbers 1 to 14.
fn text_of(n: u8) -> Option<(TextPosition, u8, bool)> {
    let n = match n {
        1..=14 => n,
        b'1'..=b'9' => n - b'0',
        b'A'..=b'E' => n - b'A' + 10,
        _ => return None,
    };
    Some(match n {
        1 => (TextPosition::None, 0, true),
        2 => (TextPosition::Below, 0, true),
        3 => (TextPosition::None, 0, false),
        4 => (TextPosition::Below, 0, false),
        5 | 6 => (TextPosition::Above, 0, n == 5),
        7 | 8 => (TextPosition::Both, 0, n == 7),
        9 | 10 => (TextPosition::Below, 1, n == 9),
        11 | 12 => (TextPosition::Above, 1, n == 11),
        _ => (TextPosition::Both, 1, n == 13),
    })
}

/// How many dots wide the narrowest bar is, for a mode of `ESC b`.
fn module_width(symbology: Symbology, mode: u8) -> Option<u8> {
    let mode = match mode {
        1..=9 => mode,
        b'1'..=b'9' => mode - b'0',
        _ => return None,
    };
    let table: &[u8] = match symbology {
        Symbology::Code39 | Symbology::Codabar => &[2, 3, 4, 2, 3, 4, 2, 3, 4],
        Symbology::Itf => &[2, 4, 6, 2, 4, 6, 2, 3, 4],
        Symbology::Gs1_128
        | Symbology::Gs1DataBarOmnidirectional
        | Symbology::Gs1DataBarTruncated
        | Symbology::Gs1DataBarLimited
        | Symbology::Gs1DataBarExpanded => &[1, 2, 3, 4, 5, 6],
        _ => &[2, 3, 4],
    };
    table.get(usize::from(mode - 1)).copied()
}

/// Whether a barcode has as many characters as its kind takes, and only those it knows.
fn data_fits(symbology: Symbology, data: &[u8]) -> bool {
    let digits = data.iter().all(u8::is_ascii_digit);
    match symbology {
        Symbology::UpcE | Symbology::UpcA => digits && (11..=12).contains(&data.len()),
        Symbology::Ean8 => digits && (7..=8).contains(&data.len()),
        Symbology::Ean13 => digits && (12..=13).contains(&data.len()),
        Symbology::Gs1DataBarOmnidirectional
        | Symbology::Gs1DataBarTruncated
        | Symbology::Gs1DataBarLimited => digits && data.len() == 13,
        Symbology::Gs1_128 | Symbology::Gs1DataBarExpanded => (2..=255).contains(&data.len()),
        _ => !data.is_empty(),
    }
}

/// What a Code 128 or Code 93 barcode says. Control characters cannot be sent as they are, so a
/// percent sign and a character stand for them: `%0` is a percent sign, `%@` to `%_` are the
/// control codes 0 to 31, `%5` is DEL, `%1` to `%4` are function characters and `%6` to `%8`
/// pick the start code. Those that are not characters are left out.
fn percent_text(raw: &[u8]) -> String {
    let mut text = String::new();
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte == b'%' && index + 1 < raw.len() {
            match raw[index + 1] {
                b'0' => text.push('%'),
                code @ b'@'..=b'_' => text.push(char::from(code - b'@')),
                b'5' => text.push('\u{7f}'),
                // The function characters, and the start codes.
                b'1'..=b'4' | b'6'..=b'8' => {}
                other => {
                    text.push('%');
                    text.push(char::from(other));
                }
            }
            index += 2;
        } else {
            text.push(char::from(byte));
            index += 1;
        }
    }
    text
}

impl Star {
    /// `ESC b n1 n2 n3 n4 d1...dk RS` prints a barcode: n1 says which, n2 where the text goes, n3
    /// how wide the bars are and n4 how high.
    pub(super) fn barcode(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let bytes = input.bytes;
        if bytes.len() < 6 {
            return Step::Incomplete;
        }
        let searched = &bytes[6..bytes.len().min(6 + MAX_BARCODE_BYTES)];
        let Some(end) = searched.iter().position(|byte| *byte == RS) else {
            if bytes.len() > 6 + MAX_BARCODE_BYTES {
                return input.unknown_run(events);
            }
            return Step::Incomplete;
        };
        let length = 6 + end + 1;
        let raw = bytes[6..6 + end].to_vec();

        let parsed = symbology(bytes[2]).and_then(|symbology| {
            let (text, font, _) = text_of(bytes[3])?;
            let width = module_width(symbology, bytes[4])?;
            (bytes[5] != 0 && data_fits(symbology, &raw)).then_some((symbology, text, font, width))
        });
        let Some((symbology, text, font, width)) = parsed else {
            return input.fixed_unknown(length, events);
        };
        machine.flush_line(events);
        let data = match symbology {
            Symbology::Code128 | Symbology::Code93 | Symbology::Gs1_128 => percent_text(&raw),
            _ => String::from_utf8_lossy(&raw).into_owned(),
        };
        events.push(Event::Barcode(Barcode {
            symbology,
            data,
            raw,
            height: bytes[5],
            width,
            text,
            font,
        }));
        Step::Consumed(length)
    }

    /// `ESC GS x` and the letter after it: the commands of PDF417. `S` sets the size, the level of
    /// error correction and the module, `D` gives the data and `P` prints it. `I` asks whether it
    /// can be printed, which no answer is given to without an encoder.
    pub(super) fn pdf417(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        match input.get(3) {
            None => Step::Incomplete,
            Some(b'S') => self.pdf417_setting(input, events),
            Some(b'D') => self.code_data(input, events, 6, |star, data| {
                if (1..=1024).contains(&data.len()) {
                    star.mode.pdf417.data = data.to_vec();
                }
            }),
            Some(b'P') => {
                self.print_pdf417(machine, events);
                Step::Consumed(4)
            }
            Some(b'I') => Step::Consumed(4),
            Some(_) => input.unknown_run(events),
        }
    }

    fn pdf417_setting(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(which) = input.get(4) else {
            return Step::Incomplete;
        };
        let length = if which == b'0' { 8 } else { 6 };
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        let value = input.bytes[5];
        let settings = &mut self.mode.pdf417;
        let valid = match which {
            b'0' => {
                let (first, second) = (input.bytes[6], input.bytes[7]);
                let by_rows = (first == 0 || (3..=90).contains(&first))
                    && (second == 0 || (1..=30).contains(&second))
                    && (first != 0 || second != 0)
                    && u32::from(first) * u32::from(second) <= 928;
                let by_proportion = (1..=99).contains(&first) && (1..=99).contains(&second);
                let valid = match value {
                    0 => by_proportion,
                    1 => by_rows,
                    _ => false,
                };
                if valid {
                    (settings.size_mode, settings.first, settings.second) = (value, first, second);
                }
                valid
            }
            b'1' if value <= 8 => {
                settings.level = value;
                true
            }
            b'2' if (1..=10).contains(&value) => {
                settings.module_width = value;
                true
            }
            b'3' if (1..=10).contains(&value) => {
                settings.module_ratio = value;
                true
            }
            _ => false,
        };
        if valid {
            Step::Consumed(length)
        } else {
            input.fixed_unknown(length, events)
        }
    }

    fn print_pdf417(&mut self, machine: &mut Machine, events: &mut Vec<Event>) {
        machine.flush_line(events);
        let settings = &self.mode.pdf417;
        let by_rows = settings.size_mode == 1;
        events.push(Event::Pdf417(Pdf417 {
            data: String::from_utf8_lossy(&settings.data).into_owned(),
            columns: if by_rows { settings.second } else { 0 },
            rows: if by_rows { settings.first } else { 0 },
            width: settings.module_width,
            height: settings.module_width.saturating_mul(settings.module_ratio),
            level_mode: 48,
            level: settings.level,
            truncated: false,
        }));
    }

    /// `ESC GS y` and the letter after it: the commands of QR codes, as those of PDF417 are.
    pub(super) fn qr(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        match input.get(3) {
            None => Step::Incomplete,
            Some(b'S') => self.qr_setting(input, events),
            Some(b'D') => self.qr_data(input, events),
            Some(b'P') => {
                machine.flush_line(events);
                let qr = &self.mode.qr;
                events.push(Event::Qr(Qr {
                    data: String::from_utf8_lossy(&qr.data).into_owned(),
                    model: qr.model,
                    size: qr.cell,
                    level: qr.level,
                }));
                Step::Consumed(4)
            }
            Some(b'I') => Step::Consumed(4),
            Some(_) => input.unknown_run(events),
        }
    }

    fn qr_setting(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let (Some(which), Some(value)) = (input.get(4), input.get(5)) else {
            return Step::Incomplete;
        };
        let qr = &mut self.mode.qr;
        let valid = match which {
            b'0' if (1..=2).contains(&value) => {
                qr.model = value;
                true
            }
            b'1' if value <= 3 => {
                qr.level = [QrLevel::L, QrLevel::M, QrLevel::Q, QrLevel::H][usize::from(value)];
                true
            }
            b'2' if (1..=8).contains(&value) => {
                qr.cell = value;
                true
            }
            _ => false,
        };
        if valid {
            Step::Consumed(6)
        } else {
            input.fixed_unknown(6, events)
        }
    }

    /// `ESC GS y D 1 m nL nH d...` gives the data as it is, and `ESC GS y D 2 a` and a blocks of
    /// `m nL nH d...` gives it by kind. The kinds do not change what is encoded here, so the data
    /// is the blocks put together.
    fn qr_data(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(kind) = input.get(4) else {
            return Step::Incomplete;
        };
        match kind {
            b'1' => self.code_data(input, events, 8, |star, data| {
                if (1..=7089).contains(&data.len()) {
                    star.mode.qr.data = data.to_vec();
                }
            }),
            b'2' => {
                let Some(blocks) = input.get(5) else {
                    return Step::Incomplete;
                };
                let mut data = Vec::new();
                let mut index = 6;
                for _ in 0..blocks {
                    let (Some(kind), Some(low), Some(high)) =
                        (input.get(index), input.get(index + 1), input.get(index + 2))
                    else {
                        return Step::Incomplete;
                    };
                    let count = usize::from(low) + usize::from(high) * 256;
                    if !(1..=4).contains(&kind) || count == 0 || index + 3 + count > MAX_QR_BYTES {
                        return input.fixed_unknown(index.min(input.bytes.len()), events);
                    }
                    if input.bytes.len() < index + 3 + count {
                        return Step::Incomplete;
                    }
                    data.extend_from_slice(&input.bytes[index + 3..index + 3 + count]);
                    index += 3 + count;
                }
                let valid = blocks != 0 && data.len() <= 7089;
                self.mode.qr.data = if valid { data } else { Vec::new() };
                Step::Consumed(index)
            }
            _ => input.unknown_run(events),
        }
    }

    /// A command that gives data by its length, in the two bytes before it. `keep` takes the data
    /// and keeps it if it is a length the command allows. Data that is not is thrown away, and is
    /// not text either.
    fn code_data(
        &mut self,
        input: &Input,
        events: &mut Vec<Event>,
        header: usize,
        keep: impl FnOnce(&mut Self, &[u8]),
    ) -> Step {
        let (Some(low), Some(high)) = (input.get(header - 2), input.get(header - 1)) else {
            return Step::Incomplete;
        };
        let count = usize::from(low) + usize::from(high) * 256;
        if header + count > MAX_QR_BYTES {
            return input.fixed_unknown(header, events);
        }
        if input.bytes.len() < header + count {
            return Step::Incomplete;
        }
        keep(self, &input.bytes[header..header + count]);
        Step::Consumed(header + count)
    }
}

/// The most data a code takes, so a length of 65535 cannot make the decoder wait for long.
const MAX_QR_BYTES: usize = 8192;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percent_stands_for_a_character_that_cannot_be_sent() {
        assert_eq!(percent_text(b"AB%0C"), "AB%C");
        assert_eq!(percent_text(b"%@%A%_"), "\0\u{1}\u{1f}");
        assert_eq!(percent_text(b"x%5"), "x\u{7f}");
        // Function characters and start codes are no characters, and an unknown pair stays.
        assert_eq!(percent_text(b"%7a%1b%8c%zd"), "abc%zd");
        assert_eq!(percent_text(b"a%"), "a%");
    }

    #[test]
    fn each_kind_has_its_numbers_of_modes() {
        assert_eq!(module_width(Symbology::Ean13, 1), Some(2));
        assert_eq!(module_width(Symbology::Ean13, b'3'), Some(4));
        assert_eq!(module_width(Symbology::Ean13, 4), None);
        assert_eq!(module_width(Symbology::Code39, 9), Some(4));
        assert_eq!(module_width(Symbology::Itf, 7), Some(2));
        assert_eq!(module_width(Symbology::Itf, 8), Some(3));
        assert_eq!(module_width(Symbology::Gs1_128, 6), Some(6));
        assert_eq!(module_width(Symbology::Gs1_128, 7), None);
    }

    #[test]
    fn the_text_of_a_barcode_goes_where_the_number_says() {
        assert_eq!(text_of(1), Some((TextPosition::None, 0, true)));
        assert_eq!(text_of(b'2'), Some((TextPosition::Below, 0, true)));
        assert_eq!(text_of(4), Some((TextPosition::Below, 0, false)));
        assert_eq!(text_of(5), Some((TextPosition::Above, 0, true)));
        assert_eq!(text_of(8), Some((TextPosition::Both, 0, false)));
        assert_eq!(text_of(9), Some((TextPosition::Below, 1, true)));
        assert_eq!(text_of(b'E'), Some((TextPosition::Both, 1, false)));
        assert_eq!(text_of(0), None);
        assert_eq!(text_of(15), None);
    }

    #[test]
    fn the_data_of_a_barcode_has_the_digits_its_kind_takes() {
        assert!(data_fits(Symbology::UpcA, b"01234567890"));
        assert!(!data_fits(Symbology::UpcA, b"0123456789"));
        assert!(!data_fits(Symbology::Ean13, b"12345678901x"));
        assert!(data_fits(Symbology::Code128, b"a"));
        assert!(!data_fits(Symbology::Code128, b""));
        assert!(data_fits(Symbology::Gs1DataBarLimited, b"1234567890123"));
    }
}
