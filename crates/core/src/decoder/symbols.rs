//! Barcodes, and the two dimensional codes of `GS ( k`.

use super::{Decoder, Frame, Input, Step, MAX_BARCODE_BYTES};
use crate::event::{Barcode, Event, Pdf417, Qr, QrLevel, Symbol, SymbolKind, Symbology};

/// `GS ( k` picks a symbol with its first byte.
const PDF417_CODE: u8 = 48;
const QR_CODE: u8 = 49;

/// The functions of `GS ( k` that every symbol has: the data is stored, then printed.
const STORE: u8 = 80;
const PRINT: u8 = 81;

/// The symbologies of the barcode command with a length, in the order of m from 65.
const WITH_LENGTH: [Symbology; 14] = [
    Symbology::UpcA,
    Symbology::UpcE,
    Symbology::Ean13,
    Symbology::Ean8,
    Symbology::Code39,
    Symbology::Itf,
    Symbology::Codabar,
    Symbology::Code93,
    Symbology::Code128,
    Symbology::Gs1_128,
    Symbology::Gs1DataBarOmnidirectional,
    Symbology::Gs1DataBarTruncated,
    Symbology::Gs1DataBarLimited,
    Symbology::Gs1DataBarExpanded,
];

/// The text of Code 128 data. The data starts with a character set, `{A`, `{B` or `{C`, and may
/// change it later. Set C is pairs of digits, one byte of 0 to 99 for each pair. `{S` shifts for
/// one character, `{1` to `{4` are the function characters, and `{{` is a brace.
fn code128_text(raw: &[u8]) -> String {
    let mut text = String::new();
    let mut set_c = false;
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte == b'{' && index + 1 < raw.len() {
            match raw[index + 1] {
                b'A' | b'B' | b'S' => set_c = false,
                b'C' => set_c = true,
                b'{' => text.push('{'),
                b'1'..=b'4' => {}
                other => {
                    text.push('{');
                    text.push(char::from(other));
                }
            }
            index += 2;
            continue;
        }
        if set_c && byte <= 99 {
            text.push_str(&format!("{byte:02}"));
        } else {
            text.push(char::from(byte));
        }
        index += 1;
    }
    text
}

fn barcode_text(symbology: Symbology, raw: &[u8]) -> String {
    match symbology {
        Symbology::Code128 | Symbology::Gs1_128 => code128_text(raw),
        _ => String::from_utf8_lossy(raw).into_owned(),
    }
}

impl Decoder {
    /// `GS k m d... NUL` for the older forms and `GS k m n d...` for the ones with a length.
    pub(super) fn parse_barcode(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let bytes = input.bytes;
        let mode = bytes[2];
        let (symbology, data_start, data_end, length) = if mode <= 6 {
            let searched = &bytes[3..bytes.len().min(3 + MAX_BARCODE_BYTES)];
            let Some(end) = searched.iter().position(|byte| *byte == 0) else {
                if bytes.len() > 3 + MAX_BARCODE_BYTES {
                    return input.unknown_run(events);
                }
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
        } else if (65..65 + WITH_LENGTH.len() as u8).contains(&mode) {
            let Some(&count) = bytes.get(3) else {
                return Step::Incomplete;
            };
            let length = 4 + usize::from(count);
            if bytes.len() < length {
                return Step::Incomplete;
            }
            (WITH_LENGTH[usize::from(mode - 65)], 4, length, length)
        } else {
            return input.unknown_run(events);
        };

        let raw = bytes[data_start..data_end].to_vec();
        self.flush_line(events);
        events.push(Event::Barcode(Barcode {
            symbology,
            data: barcode_text(symbology, &raw),
            raw,
            height: self.state.barcode_height,
            width: self.state.barcode_width,
            text: self.state.barcode_text,
            font: self.state.barcode_font,
        }));
        Step::Consumed(length)
    }

    /// `GS ( k pL pH cn fn ...` stores the settings and the data of a two dimensional code, then
    /// prints it.
    pub(super) fn parse_symbol(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let length = match input.frame(3, 2) {
            Frame::Need | Frame::Waiting => return Step::Incomplete,
            Frame::TooBig => return input.unknown_run(events),
            Frame::Whole(length) => length,
        };
        // A symbol and a function are at least two bytes, so the length must be at least 2.
        if length < 7 {
            return input.framed_unknown(3, 2, events);
        }
        let (symbol, function) = (input.bytes[5], input.bytes[6]);
        let params = &input.bytes[7..length];

        let kind = match symbol {
            PDF417_CODE | QR_CODE => None,
            50 => Some(SymbolKind::MaxiCode),
            51 => Some(SymbolKind::Gs1DataBar),
            52 => Some(SymbolKind::Composite),
            53 => Some(SymbolKind::Aztec),
            54 => Some(SymbolKind::DataMatrix),
            _ => return input.framed_unknown(3, 2, events),
        };

        match function {
            // The data comes after a byte that is always 48.
            STORE => {
                let data = params.get(1..).unwrap_or(&[]).to_vec();
                self.state.symbol_data.insert(symbol, data);
            }
            PRINT => {
                self.flush_line(events);
                let data = self
                    .state
                    .symbol_data
                    .get(&symbol)
                    .cloned()
                    .unwrap_or_default();
                events.push(match kind {
                    None if symbol == QR_CODE => Event::Qr(self.qr(data)),
                    None => Event::Pdf417(self.pdf417(data)),
                    Some(kind) => Event::Symbol(Symbol {
                        kind,
                        data,
                        settings: self
                            .state
                            .symbol_settings
                            .get(&symbol)
                            .map(|settings| {
                                settings
                                    .iter()
                                    .map(|(function, bytes)| (*function, bytes.clone()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }),
                });
            }
            // 82 asks how big the symbol would be, which only a printer that draws it can say. The
            // others, from 65 up, are the settings of the symbol.
            65..=79 => {
                self.state
                    .symbol_settings
                    .entry(symbol)
                    .or_default()
                    .insert(function, params.to_vec());
            }
            82 => {}
            _ => return input.framed_unknown(3, 2, events),
        }
        Step::Consumed(length)
    }

    fn setting(&self, symbol: u8, function: u8) -> &[u8] {
        self.state
            .symbol_settings
            .get(&symbol)
            .and_then(|settings| settings.get(&function))
            .map_or(&[], Vec::as_slice)
    }

    fn qr(&self, data: Vec<u8>) -> Qr {
        let first = |function: u8| self.setting(QR_CODE, function).first().copied();
        Qr {
            data: String::from_utf8_lossy(&data).into_owned(),
            model: match first(65) {
                Some(49) => 1,
                Some(51) => 3,
                _ => 2,
            },
            size: first(67).unwrap_or(3),
            level: match first(69) {
                Some(49) => QrLevel::M,
                Some(50) => QrLevel::Q,
                Some(51) => QrLevel::H,
                _ => QrLevel::L,
            },
        }
    }

    fn pdf417(&self, data: Vec<u8>) -> Pdf417 {
        let first = |function: u8| self.setting(PDF417_CODE, function).first().copied();
        let level = self.setting(PDF417_CODE, 69);
        Pdf417 {
            data: String::from_utf8_lossy(&data).into_owned(),
            columns: first(65).unwrap_or(0),
            rows: first(66).unwrap_or(0),
            width: first(67).unwrap_or(3),
            height: first(68).unwrap_or(3),
            level_mode: level.first().copied().unwrap_or(48),
            level: level.get(1).copied().unwrap_or(0),
            truncated: matches!(first(70), Some(1 | 49)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_128_text_follows_the_character_sets() {
        assert_eq!(code128_text(b"{BABC-123"), "ABC-123");
        // "No." in set B, then 12 34 56 in set C.
        assert_eq!(
            code128_text(&[b'{', b'B', b'N', b'o', b'.', b'{', b'C', 12, 34, 56]),
            "No.123456"
        );
        assert_eq!(code128_text(b"{B{{x"), "{x");
        assert_eq!(code128_text(b"{B{1A{2"), "A");
        assert_eq!(code128_text(b"{C\x00\x63"), "0099");
        // A brace that is not followed by a known letter is kept.
        assert_eq!(code128_text(b"{Bx{z"), "x{z");
        assert_eq!(code128_text(b""), "");
        assert_eq!(code128_text(b"{"), "{");
    }
}
