//! Fonts, code pages, characters and how they look.

use super::super::machine::Machine;
use super::super::reader::{Input, Step};
use super::Star;
use crate::dialect::Dialect;
use crate::event::{Event, Style, UserCharacter};

/// The code page that `ESC GS t n` selects, by the name that [`crate::codepage`] reads. The pages
/// that only Star has, and the Thai and Vietnamese ones, have no table of their own there and are
/// read as Windows 1252, or as the closest page. The numbers that are not a page give nothing, and
/// so do 128, which is UTF-8, and 255, a page the program draws itself.
pub(in crate::decoder) fn code_page(n: u8) -> Option<&'static str> {
    Some(match n {
        0 | 1 | 3 => "CP437",
        // The Japanese katakana page.
        2 => "CP932",
        4 => "CP858",
        5 => "CP852",
        6 => "CP860",
        7 => "CP861",
        8 => "CP863",
        9 => "CP865",
        10 => "CP866",
        11 => "CP855",
        12 => "CP857",
        13 => "CP862",
        14 => "CP864",
        15 => "CP737",
        16 => "CP851",
        17 => "CP869",
        18 => "CP928",
        19 => "CP772",
        20 => "CP774",
        21 => "CP874",
        32 => "CP1252",
        33 => "CP1250",
        34 => "CP1251",
        64 => "CP3840",
        65 => "CP3841",
        66 => "CP3843",
        67 => "CP3844",
        68 => "CP3845",
        69 => "CP3846",
        70 => "CP3847",
        71 => "CP3848",
        72 => "CP1001",
        73 => "CP2001",
        74 => "CP3001",
        75 => "CP3002",
        76 => "CP3011",
        77 => "CP3012",
        78 => "CP3021",
        79 => "CP3041",
        96 => "THAI42",
        97 => "THAI11",
        98 => "THAI13",
        99 => "THAI14",
        100 => "THAI16",
        101 => "THAI17",
        102 => "THAI18",
        110 => "TCVN-3-1",
        111 => "TCVN-3-2",
        _ => return None,
    })
}

/// A number, or its digit, that is at most `max`: 0 to 5 may also be written '0' to '5'.
pub(super) fn number(n: u8, max: u8) -> Option<u8> {
    match n {
        0..=15 if n <= max => Some(n),
        b'0'..=b'9' if n - b'0' <= max => Some(n - b'0'),
        _ => None,
    }
}

/// A number of dots, written 0 to 15 or as the hexadecimal digit `0` to `F`.
fn dots(n: u8) -> Option<u8> {
    match n {
        0..=15 => Some(n),
        b'0'..=b'9' => Some(n - b'0'),
        b'A'..=b'F' => Some(n - b'A' + 10),
        _ => None,
    }
}

/// The twelve by twenty four dots of a drawn character, which come as 24 rows of two bytes, as
/// the 36 bytes of 12 columns of 3 bytes that [`UserCharacter`] has.
fn columns_of(rows: &[u8]) -> Vec<u8> {
    let mut columns = vec![0u8; 12 * 3];
    for (row, pair) in rows.as_chunks::<2>().0.iter().enumerate() {
        for x in 0..12 {
            let byte = pair[x / 8];
            if byte & (0x80 >> (x % 8)) != 0 {
                columns[x * 3 + row / 8] |= 0x80 >> (row % 8);
            }
        }
    }
    columns
}

impl Star {
    /// The commands of text: the font, the code page, the sizes and the looks of characters, and
    /// the Kanji. Gives nothing for a command that is not one of those.
    pub(super) fn text_command(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        code: u8,
        events: &mut Vec<Event>,
    ) -> Option<Step> {
        let line = self.dialect == Dialect::StarLine;
        Some(match code {
            0x52 => self.argument(machine, input, events, 2, Self::international),
            0x2f => self.argument(machine, input, events, 2, Self::slash_zero),
            0x20 => self.argument(machine, input, events, 2, Self::space),
            0x57 => self.argument(machine, input, events, 2, Self::wider),
            0x68 => self.argument(machine, input, events, 2, Self::higher),
            0x2d => self.argument(machine, input, events, 2, Self::underline),
            0x5f => self.argument(machine, input, events, 2, Self::overline),
            0x24 => self.argument(machine, input, events, 2, Self::shift_jis),
            0x69 => self.arguments(machine, input, events, Self::size),
            0x73 | 0x74 => self.arguments(machine, input, events, Self::kanji_space),
            // The pitches, which are the room right of a character, and the height of a character
            // that goes with `SO` and `DC4`.
            0x4d if line => self.style(machine, events, 2, |style| style.spacing = 0),
            0x67 if line => self.style(machine, events, 2, |style| style.spacing = 2),
            0x50 if line => self.style(machine, events, 2, |style| style.spacing = 3),
            0x3a if line => self.style(machine, events, 2, |style| style.spacing = 4),
            0x0e if line => self.style(machine, events, 2, |style| style.height = 2),
            0x14 if line => self.style(machine, events, 2, |style| style.height = 1),
            0x45 => self.style(machine, events, 2, |style| style.bold = true),
            0x46 => self.style(machine, events, 2, |style| style.bold = false),
            0x34 => self.style(machine, events, 2, |style| style.invert = true),
            0x35 => self.style(machine, events, 2, |style| style.invert = false),
            0x70 => self.kanji_mode(machine, events, |kanji| kanji.jis_mode(true)),
            0x71 => self.kanji_mode(machine, events, |kanji| kanji.jis_mode(false)),
            _ => return None,
        })
    }

    /// Reads the byte at `at`, after the name of a command, and does what the command says with it.
    /// The command says whether the byte is one it allows, and is reported as unknown if not.
    pub(super) fn argument(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
        at: usize,
        command: fn(&mut Self, &mut Machine, u8, &mut Vec<Event>) -> bool,
    ) -> Step {
        let Some(n) = input.get(at) else {
            return Step::Incomplete;
        };
        if command(self, machine, n, events) {
            Step::Consumed(at + 1)
        } else {
            input.fixed_unknown(at + 1, events)
        }
    }

    /// The same for a command with two bytes after it.
    fn arguments(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
        command: fn(&mut Self, &mut Machine, u8, u8, &mut Vec<Event>) -> bool,
    ) -> Step {
        let (Some(first), Some(second)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        if command(self, machine, first, second, events) {
            Step::Consumed(4)
        } else {
            input.fixed_unknown(4, events)
        }
    }

    /// Changes how the next characters look, in a command that is `length` bytes long.
    pub(super) fn style(
        &mut self,
        machine: &mut Machine,
        events: &mut Vec<Event>,
        length: usize,
        change: impl FnOnce(&mut Style),
    ) -> Step {
        machine.change_style(events, change);
        Step::Consumed(length)
    }

    fn kanji_mode(
        &mut self,
        machine: &mut Machine,
        events: &mut Vec<Event>,
        change: impl FnOnce(&mut crate::decoder::kanji::Kanji),
    ) -> Step {
        machine.flush_line(events);
        change(&mut machine.state.kanji);
        Step::Consumed(2)
    }

    /// `ESC R n` picks an international character set. 0 to 13 are the sets the languages share
    /// with ESC/POS. Ireland and the legal set have no table here and are read as the American set.
    fn international(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let set = match n {
            0..=13 => n,
            b'0'..=b'9' => n - b'0',
            b'A'..=b'D' => n - b'A' + 10,
            14 | 64 | b'E' => 0,
            _ => return false,
        };
        machine.flush_line(events);
        machine.state.international = set;
        true
    }

    /// `ESC / n` writes the zero with a slash, which nothing here draws differently.
    fn slash_zero(&mut self, _machine: &mut Machine, n: u8, _events: &mut Vec<Event>) -> bool {
        matches!(n, 0 | 1 | b'0' | b'1')
    }

    /// `ESC SP n` sets the room right of each character, in dots.
    fn space(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(space) = dots(n) else {
            return false;
        };
        machine.change_style(events, |style| style.spacing = space);
        true
    }

    /// `ESC W n` sets how many times wider characters are, from 1 to 6.
    fn wider(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(times) = number(n, 5) else {
            return false;
        };
        machine.change_style(events, |style| style.width = times + 1);
        true
    }

    /// `ESC h n` sets how many times higher characters are, from 1 to 6.
    fn higher(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(times) = number(n, 5) else {
            return false;
        };
        machine.change_style(events, |style| style.height = times + 1);
        true
    }

    /// `ESC i n1 n2` sets the height and then the width, as `ESC h` and `ESC W` do.
    fn size(&mut self, machine: &mut Machine, high: u8, wide: u8, events: &mut Vec<Event>) -> bool {
        let (Some(high), Some(wide)) = (number(high, 5), number(wide, 5)) else {
            return false;
        };
        machine.change_style(events, |style| {
            style.height = high + 1;
            style.width = wide + 1;
        });
        true
    }

    /// `ESC - n` turns the underline on and off.
    fn underline(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(on) = number(n, 1) else {
            return false;
        };
        machine.change_style(events, |style| style.underline = on);
        true
    }

    /// `ESC _ n` turns the line over the characters on and off.
    fn overline(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(on) = number(n, 1) else {
            return false;
        };
        machine.change_style(events, |style| style.overline = on == 1);
        true
    }

    /// `ESC $ n` turns the Shift JIS Kanji mode on and off.
    fn shift_jis(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let Some(on) = number(n, 1) else {
            return false;
        };
        machine.flush_line(events);
        machine.state.kanji.shift_jis_mode(on == 1);
        true
    }

    /// `ESC s n1 n2` and `ESC t n1 n2` set the room left and right of full and half size Kanji,
    /// which nothing here draws differently.
    fn kanji_space(
        &mut self,
        _machine: &mut Machine,
        left: u8,
        right: u8,
        _events: &mut Vec<Event>,
    ) -> bool {
        number(left, 7).is_some() && dots(right).is_some()
    }

    /// `ESC RS F n` picks the font: 0 is A, 1 is B and 16 is the OCR font.
    pub(super) fn select_font(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        let font = match n {
            0 | 1 => n,
            16 => 2,
            _ => return false,
        };
        machine.change_style(events, |style| style.font = font);
        events.push(Event::Font(font));
        true
    }

    /// `ESC GS t n` picks the code page. 128 is UTF-8, which the text is then read in.
    pub(super) fn select_code_page(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        if code_page(n).is_none() && !matches!(n, 128 | 255) {
            return false;
        }
        machine.flush_line(events);
        machine.state.codepage = n;
        machine.state.utf8 = n == 128;
        true
    }

    /// `ESC GS b n` smooths the edges of characters.
    pub(super) fn smoothing(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some(on) = number(n, 1) else {
            return false;
        };
        machine.change_style(events, |style| style.smooth = on == 1);
        true
    }

    /// `ESC GS = n1 n2 d...` writes a page the program draws, with n1 + n2 x 256 bytes of dots.
    pub(super) fn blank_code_page(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        input.framed_ignored(3, 2, events)
    }

    /// `ESC & c1 c2 n d1...d48` draws a character of 12 by 24 dots to print where the code n is,
    /// and `ESC & c1 c2 n` takes it back.
    pub(super) fn download_characters(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(first), Some(second), Some(code)) = (input.get(2), input.get(3), input.get(4))
        else {
            return Step::Incomplete;
        };
        if !matches!(first, 1 | b'1') || !(32..=127).contains(&code) {
            return input.fixed_unknown(5, events);
        }
        match second {
            0 | b'0' => {
                machine.flush_line(events);
                if machine.state.user_defined.remove(&code) {
                    events.push(Event::DeleteCharacter(code));
                }
                Step::Consumed(5)
            }
            1 | b'1' => {
                if input.bytes.len() < 5 + 48 {
                    return Step::Incomplete;
                }
                machine.flush_line(events);
                events.push(Event::DefineCharacter(UserCharacter {
                    code,
                    width: 12,
                    column_bytes: 3,
                    data: columns_of(&input.bytes[5..5 + 48]),
                }));
                machine.state.user_defined.insert(code);
                Step::Consumed(5 + 48)
            }
            _ => input.fixed_unknown(5, events),
        }
    }

    /// `ESC r c1 c2 d1...d72` draws a Kanji of 24 by 24 dots.
    pub(super) fn kanji_download(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(first), Some(second)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        let length = 4 + 72;
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        if !machine.store_user_kanji([first, second], (24, 24), &input.bytes[4..length], events) {
            return input.fixed_unknown(4, events);
        }
        Step::Consumed(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_of_code_pages_are_those_of_the_manual() {
        assert_eq!(code_page(0), Some("CP437"));
        assert_eq!(code_page(4), Some("CP858"));
        assert_eq!(code_page(32), Some("CP1252"));
        assert_eq!(code_page(34), Some("CP1251"));
        assert_eq!(code_page(77), Some("CP3012"));
        assert_eq!(code_page(128), None);
        assert_eq!(code_page(22), None);
    }

    #[test]
    fn a_number_may_be_written_as_a_digit() {
        assert_eq!(number(3, 5), Some(3));
        assert_eq!(number(b'3', 5), Some(3));
        assert_eq!(number(6, 5), None);
        assert_eq!(number(b'6', 5), None);
        assert_eq!(dots(b'F'), Some(15));
        assert_eq!(dots(b'G'), None);
    }

    #[test]
    fn rows_of_two_bytes_become_columns_of_three() {
        // The first row has its first dot on and the last of 12 dots on, the last row its first.
        let mut rows = vec![0u8; 48];
        rows[0] = 0x80;
        rows[1] = 0x10;
        rows[46] = 0x80;
        let columns = columns_of(&rows);
        assert_eq!(columns[0], 0x80);
        assert_eq!(columns[2], 0x01);
        assert_eq!(columns[11 * 3], 0x80);
        assert_eq!(columns.iter().filter(|b| **b != 0).count(), 3);
    }
}
