//! Barcodes, which `GS k` prints, and what `GS h`, `GS w`, `GS H` and `GS f` set for the next one.

use super::super::machine::Machine;
use super::super::reader::{Input, Step, MAX_BARCODE_BYTES};
use super::EscPos;
use crate::event::{Barcode, Event, Symbology, TextPosition};

/// What the next barcode is made with.
#[derive(Debug, Clone)]
pub(super) struct Settings {
    pub(super) height: u8,
    pub(super) width: u8,
    pub(super) text: TextPosition,
    pub(super) font: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            height: 162,
            width: 3,
            text: TextPosition::None,
            font: 0,
        }
    }
}

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

impl EscPos {
    /// `GS k m d... NUL` for the older forms and `GS k m n d...` for the ones with a length.
    pub(super) fn parse_barcode(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
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
        machine.flush_line(events);
        events.push(Event::Barcode(Barcode {
            symbology,
            data: barcode_text(symbology, &raw),
            raw,
            height: self.barcode.height,
            width: self.barcode.width,
            text: self.barcode.text,
            font: self.barcode.font,
        }));
        Step::Consumed(length)
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
