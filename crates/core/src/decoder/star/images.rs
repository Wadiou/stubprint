//! Pictures: bit images, logos, and raster graphics.

use super::super::machine::Machine;
use super::super::pictures::print_mode;
use super::super::reader::{Input, Step, MAX_COMMAND_BYTES};
use super::Star;
use crate::bitmap::Bitmap;
use crate::event::{CutMode, Event, Feed, FeedAmount};

/// How many rows raster mode collects before it prints them, so a program that never ends its
/// picture cannot make the decoder grow without end.
const MAX_RASTER_ROWS: usize = 16 * 1024;
/// What `ESC * r E` and `ESC * r F` do when nothing was set: feed to the next page and cut fully.
const DEFAULT_END_MODE: u16 = 9;

/// The picture that raster mode is drawing: rows of dots, each as many bytes as it needs.
#[derive(Debug, Clone)]
pub(super) struct Raster {
    rows: Vec<Vec<u8>>,
    /// The row that `k` adds to and `b` finishes.
    current: Vec<u8>,
    /// Bytes of room at the left of each row.
    left: usize,
    /// What `ESC FF EOT` and `ESC FF NUL` do.
    eot: u16,
    form_feed: u16,
}

impl Default for Raster {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            current: Vec::new(),
            left: 0,
            eot: DEFAULT_END_MODE,
            form_feed: DEFAULT_END_MODE,
        }
    }
}

impl Raster {
    /// Lays data in the current row, over what is there, after the left margin.
    fn add(&mut self, data: &[u8]) {
        let end = self.left + data.len();
        if self.current.len() < end {
            self.current.resize(end, 0);
        }
        for (target, byte) in self.current[self.left..end].iter_mut().zip(data) {
            *target |= byte;
        }
    }

    fn finish_row(&mut self) {
        let row = std::mem::take(&mut self.current);
        self.rows.push(row);
    }

    /// The rows as one picture, as wide as the widest, or nothing when there are none.
    fn take_picture(&mut self) -> Option<Bitmap> {
        if !self.current.is_empty() {
            self.finish_row();
        }
        let rows = std::mem::take(&mut self.rows);
        let row_bytes = rows.iter().map(Vec::len).max()?;
        if row_bytes == 0 {
            return None;
        }
        let height = rows.len() as u32;
        let data: Vec<u8> = rows
            .into_iter()
            .flat_map(|mut row| {
                row.resize(row_bytes, 0);
                row
            })
            .collect();
        Bitmap::from_packed(row_bytes, height, data)
    }
}

/// How an end of document ends: the cut it makes, from the number of `ESC * r E` and `ESC * r F`.
fn end_mode_cut(mode: u16) -> Option<CutMode> {
    match mode {
        8 | 9 | 36 | 37 => Some(CutMode::Full),
        12 | 13 => Some(CutMode::Partial),
        _ => None,
    }
}

/// Reads a number written as ASCII digits that ends in a NUL, from `at`. Gives the number and how
/// many bytes it took up to and with the NUL, or nothing when a byte is not a digit. `Incomplete`
/// is the answer while the NUL has not arrived.
fn ascii_number(input: &Input, at: usize) -> Result<Option<(u32, usize)>, ()> {
    let mut value: u32 = 0;
    for (offset, byte) in input.bytes.iter().skip(at).enumerate() {
        match byte {
            0 => return Ok(Some((value, offset + 1))),
            b'0'..=b'9' if offset < 9 => value = value * 10 + u32::from(byte - b'0'),
            _ => return Err(()),
        }
    }
    Ok(None)
}

impl Star {
    /// Prints what raster mode drew so far.
    fn raster_print(&mut self, machine: &mut Machine, events: &mut Vec<Event>) {
        let picture = self.mode.raster.as_mut().and_then(Raster::take_picture);
        if let Some(picture) = picture {
            machine.print_image(picture, events);
        }
    }

    /// `b n1 n2 d1...` adds a row of dots and goes on to the next, and `k n1 n2 d1...` adds data to
    /// the row without going on. Anything else in raster mode is ignored.
    pub(super) fn raster_byte(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let byte = input.bytes[0];
        if byte != b'b' && byte != b'k' {
            return Step::Consumed(1);
        }
        let (Some(low), Some(high)) = (input.get(1), input.get(2)) else {
            return Step::Incomplete;
        };
        let count = usize::from(low) + usize::from(high) * 256;
        if input.bytes.len() < 3 + count {
            return Step::Incomplete;
        }
        let full = {
            let Some(raster) = self.mode.raster.as_mut() else {
                return Step::Consumed(3 + count);
            };
            raster.add(&input.bytes[3..3 + count]);
            if byte == b'b' {
                raster.finish_row();
            }
            raster.rows.len() >= MAX_RASTER_ROWS
        };
        if full {
            self.raster_print(machine, events);
        }
        Step::Consumed(3 + count)
    }

    /// `ESC * r` and a letter: the commands of raster mode. A number that goes with one is written
    /// in digits and ends in a NUL.
    pub(super) fn esc_star(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(r), Some(command)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        if r != b'r' || self.dialect != crate::dialect::Dialect::StarLine {
            return input.unknown_run(events);
        }
        match command {
            b'R' => {
                if let Some(raster) = self.mode.raster.as_mut() {
                    *raster = Raster::default();
                }
                Step::Consumed(4)
            }
            b'A' => {
                machine.flush_line(events);
                self.mode.raster.get_or_insert_with(Raster::default);
                Step::Consumed(4)
            }
            b'B' => {
                self.raster_print(machine, events);
                self.mode.raster = None;
                Step::Consumed(4)
            }
            b'C' => {
                if let Some(raster) = self.mode.raster.as_mut() {
                    raster.rows.clear();
                    raster.current.clear();
                }
                Step::Consumed(4)
            }
            // Audio that is played back, which has no effect here: ESC * r S, and ESC * r s with a
            // letter and a number.
            b'S' => Step::Consumed(4),
            b's' => self.raster_with_number(machine, input, 5, events, |_, _, _, _| {}),
            // The buzzer, with the terminal before the number.
            b'V' => self.raster_with_number(machine, input, 5, events, |_, _, _, _| {}),
            b'D' | b'E' | b'F' | b'P' | b'Q' | b'T' | b'K' | b'Y' | b'N' => self
                .raster_with_number(
                    machine,
                    input,
                    4,
                    events,
                    move |star, machine, n, events| {
                        star.raster_setting(machine, command, n, events);
                    },
                ),
            b'm' => {
                let Some(side) = input.get(4) else {
                    return Step::Incomplete;
                };
                if !matches!(side, b'l' | b'r') {
                    return input.unknown_run(events);
                }
                self.raster_with_number(machine, input, 5, events, move |star, _, n, _| {
                    if side == b'l' {
                        if let Some(raster) = star.mode.raster.as_mut() {
                            raster.left = n as usize;
                        }
                    }
                })
            }
            _ => input.unknown_run(events),
        }
    }

    /// Reads the number of a command of raster mode, and does what the command says with it.
    fn raster_with_number(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        at: usize,
        events: &mut Vec<Event>,
        apply: impl FnOnce(&mut Self, &mut Machine, u32, &mut Vec<Event>),
    ) -> Step {
        match ascii_number(input, at) {
            Ok(Some((n, length))) => {
                apply(self, machine, n, events);
                Step::Consumed(at + length)
            }
            Ok(None) => Step::Incomplete,
            Err(()) => input.unknown_run(events),
        }
    }

    /// What the commands of raster mode that take a number do with it.
    fn raster_setting(
        &mut self,
        machine: &mut Machine,
        command: u8,
        n: u32,
        events: &mut Vec<Event>,
    ) {
        match command {
            // Drives the first drawer, the second, or both.
            b'D' => {
                for device in [0u8, 1] {
                    if n & (1 << device) != 0 {
                        self.drawer(machine, device, events);
                    }
                }
            }
            b'E' | b'F' => {
                let mode = if n == 0 { DEFAULT_END_MODE } else { n as u16 };
                if let Some(raster) = self.mode.raster.as_mut() {
                    if command == b'E' {
                        raster.eot = mode;
                    } else {
                        raster.form_feed = mode;
                    }
                }
            }
            // Moves down the paper by n dots, as empty rows of the picture.
            b'Y' => {
                if let Some(raster) = self.mode.raster.as_mut() {
                    raster.finish_row_if_any();
                    raster.rows.extend(std::iter::repeat_n(
                        Vec::new(),
                        (n as usize).min(MAX_RASTER_ROWS),
                    ));
                }
            }
            // The page length, quality, top margin, colour, and the bytes to discard, which does
            // nothing to what is drawn.
            _ => {}
        }
    }

    /// `ESC FF NUL` and `ESC FF EOT` end the document: they print what was drawn, feed to the
    /// next page, and cut, as `ESC * r F` and `ESC * r E` say.
    pub(super) fn raster_end(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(kind) = input.get(2) else {
            return Step::Incomplete;
        };
        let Some(raster) = self.mode.raster.as_ref() else {
            return Step::Consumed(3);
        };
        let mode = match kind {
            0 => raster.form_feed,
            4 => raster.eot,
            _ => return input.fixed_unknown(3, events),
        };
        self.raster_print(machine, events);
        events.push(Event::Feed(Feed {
            amount: FeedAmount::Page,
            reverse: false,
        }));
        if let Some(cut) = end_mode_cut(mode) {
            events.push(Event::Cut(cut));
        }
        Step::Consumed(3)
    }

    /// `ESC K`, `ESC L`, `ESC k` and `ESC X` print a picture: each dot of the data is 3 by 3 dots,
    /// 1 by 3, 1 by 1, or a column of 24, as the letter says.
    pub(super) fn bit_image(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(low), Some(high)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        let letter = input.bytes[1];
        let n = usize::from(low) + usize::from(high) * 256;
        let bytes = match letter {
            b'K' | b'L' => n,
            b'k' => n * 24,
            _ => n * 3,
        };
        if n == 0 || (letter == b'k' && high != 0) || 4 + bytes > MAX_COMMAND_BYTES {
            return input.fixed_unknown(4, events);
        }
        let length = 4 + bytes;
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        let data = &input.bytes[4..length];
        let bitmap = match letter {
            b'K' => Bitmap::from_columns(n as u32, 8, data).map(|b| b.scaled(3, 3)),
            b'L' => Bitmap::from_columns(n as u32, 8, data).map(|b| b.scaled(1, 3)),
            b'k' => Bitmap::from_packed(n, 24, data.to_vec()),
            _ => Bitmap::from_columns(n as u32, 24, data),
        };
        match bitmap {
            Some(bitmap) => machine.print_image(bitmap, events),
            None => machine.flush_line(events),
        }
        Step::Consumed(length)
    }

    /// `ESC RS L m` prints all the logos, or with m of 255 deletes them, and then resets the printer.
    pub(super) fn logo_batch(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        if n == 255 {
            machine.memory.set_bit_images(Vec::new());
        } else {
            let Some((across, down)) = print_mode(n) else {
                return false;
            };
            let logos = machine.memory.bit_images().to_vec();
            for logo in logos {
                machine.print_image(logo.scaled(across, down), events);
            }
        }
        self.initialize(machine, events);
        true
    }

    /// `ESC GS S m xL xH yL yH n d...` prints rows of dots: x bytes in a row, y rows, and the data
    /// as it is. `ESC GS X m xL xH yL yH p1 p2 p3 p4 n d...` sends the same, packed in p1 to p4
    /// bytes: runs of one byte, and runs of different bytes.
    pub(super) fn raster_graphics(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
        packed: bool,
    ) -> Step {
        let header = if packed { 13 } else { 9 };
        if input.bytes.len() < header {
            return Step::Incomplete;
        }
        let b = input.bytes;
        let row_bytes = usize::from(b[4]) + usize::from(b[5]) * 256;
        let height = usize::from(b[6]) + usize::from(b[7]) * 256;
        let sent = if packed {
            u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize
        } else {
            row_bytes * height
        };
        let (m, n) = (b[3], b[header - 1]);
        let valid = m == 1 && n == 0 && (1..=128).contains(&row_bytes) && height > 0;
        if !valid || header + sent > MAX_COMMAND_BYTES {
            return input.fixed_unknown(header, events);
        }
        if b.len() < header + sent {
            return Step::Incomplete;
        }
        let data = &b[header..header + sent];
        let dots = if packed {
            unpack(data, row_bytes * height)
        } else {
            data.to_vec()
        };
        match Bitmap::from_packed(row_bytes, height as u32, dots) {
            Some(bitmap) => machine.print_image(bitmap, events),
            None => machine.flush_line(events),
        }
        Step::Consumed(header + sent)
    }
}

impl Raster {
    fn finish_row_if_any(&mut self) {
        if !self.current.is_empty() {
            self.finish_row();
        }
    }
}

/// Unpacks data that is a header byte and then bytes: a header from 0 to 127 is followed by that
/// many plus one bytes, a header from 129 to 255 by one byte that is repeated 257 minus the header
/// times, and 128 is nothing. The result is as long as it should be: cut, or filled with zeros.
fn unpack(data: &[u8], length: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(length);
    let mut index = 0;
    while index < data.len() && out.len() < length {
        let header = data[index];
        index += 1;
        match header {
            0..=127 => {
                let count = usize::from(header) + 1;
                let end = (index + count).min(data.len());
                out.extend_from_slice(&data[index..end]);
                index = end;
            }
            128 => {}
            _ => {
                let count = 257 - usize::from(header);
                if let Some(byte) = data.get(index) {
                    out.extend(std::iter::repeat_n(*byte, count));
                }
                index += 1;
            }
        }
    }
    out.resize(length, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_data_unpacks_as_the_manual_shows() {
        // The example of the manual: 32 bytes packed in 22.
        let packed = [
            0xFD, 0x00, 0x00, 0x08, 0xFF, 0x00, 0x02, 0x80, 0x00, 0x08, 0xFF, 0x00, 0x02, 0x80,
            0x00, 0x08, 0xFF, 0x00, 0x00, 0x80, 0xF3, 0xFF,
        ];
        let unpacked = [
            0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x80, 0x00, 0x08, 0x00, 0x00, 0x80, 0x00,
            0x08, 0x00, 0x00, 0x80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert_eq!(unpack(&packed, 32), unpacked);
        // A header of 128 does nothing, and short data is filled with zeros.
        assert_eq!(unpack(&[0x80, 0x01, 0xAA, 0xBB], 4), [0xAA, 0xBB, 0, 0]);
    }

    #[test]
    fn a_number_ends_in_a_nul_and_is_digits() {
        let bytes = [b'1', b'2', 0, b'x'];
        let input = Input {
            bytes: &bytes,
            offset: 0,
            at_end: false,
        };
        assert_eq!(ascii_number(&input, 0), Ok(Some((12, 3))));
        assert_eq!(ascii_number(&input, 1), Ok(Some((2, 2))));
        let open = Input {
            bytes: &bytes[..2],
            offset: 0,
            at_end: false,
        };
        assert_eq!(ascii_number(&open, 0), Ok(None));
        let bad = Input {
            bytes: b"1x\0",
            offset: 0,
            at_end: false,
        };
        assert_eq!(ascii_number(&bad, 0), Err(()));
    }

    #[test]
    fn rows_of_raster_mode_become_one_picture() {
        let mut raster = Raster::default();
        raster.add(&[0b1000_0000]);
        raster.finish_row();
        raster.add(&[0b0000_0001, 0b1000_0000]);
        raster.finish_row();
        let picture = raster.take_picture().unwrap();
        assert_eq!((picture.width(), picture.height()), (16, 2));
        assert!(picture.is_black(0, 0));
        assert!(picture.is_black(7, 1));
        assert!(picture.is_black(8, 1));
        assert!(raster.take_picture().is_none());
    }

    #[test]
    fn k_adds_to_a_row_that_b_finishes() {
        let mut raster = Raster::default();
        raster.add(&[0b1010_0000]);
        raster.add(&[0b0101_0000]);
        raster.finish_row();
        let picture = raster.take_picture().unwrap();
        assert!(picture.is_black(0, 0) && picture.is_black(1, 0) && picture.is_black(2, 0));
    }

    #[test]
    fn the_end_modes_say_which_cut() {
        assert_eq!(end_mode_cut(9), Some(CutMode::Full));
        assert_eq!(end_mode_cut(13), Some(CutMode::Partial));
        assert_eq!(end_mode_cut(1), None);
        assert_eq!(end_mode_cut(2), None);
    }
}
