//! The commands that start with `ESC`.

use super::{Decoder, Frame, Input, Step, MAX_COMMAND_BYTES};
use crate::bitmap::Bitmap;
use crate::event::{Align, CutMode, Event, LineSpacing, Page, Position, Pulse, UserCharacter};

/// The most tab stops a printer remembers.
const MAX_TAB_STOPS: usize = 32;

/// Reads a list of stops that starts after the two bytes of a command, such as `ESC D`: numbers that
/// go up, ended by a NUL. A byte that breaks the order, or the one after the last stop there is
/// room for, ends the list and is left to be read as data. Returns the stops and how many bytes
/// the command took, or nothing if the list is not all here.
pub(super) fn read_stops(input: &Input, most: usize) -> Option<(Vec<u8>, usize)> {
    let mut stops: Vec<u8> = Vec::new();
    let mut index = 2;
    loop {
        let next = input.get(index)?;
        if next == 0 {
            index += 1;
            break;
        }
        if stops.last().is_some_and(|last| next <= *last) || stops.len() == most {
            break;
        }
        stops.push(next);
        index += 1;
    }
    Some((stops, index))
}

impl Decoder {
    pub(super) fn parse_esc(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(code) = input.get(1) else {
            return Step::Incomplete;
        };
        match code {
            // ESC @ resets the printer.
            0x40 => {
                self.initialize(events);
                Step::Consumed(2)
            }
            // ESC 2 selects the default line spacing.
            0x32 => {
                self.flush_line(events);
                events.push(Event::LineSpacing(LineSpacing::Default));
                Step::Consumed(2)
            }
            // ESC i and ESC m are the cuts of older printers. Both leave a part of the paper uncut.
            0x69 | 0x6d => {
                self.flush_line(events);
                events.push(Event::Cut(CutMode::Partial));
                Step::Consumed(2)
            }
            // ESC L and ESC S pick page mode and standard mode, and ESC FF prints the page.
            0x4c => {
                self.flush_line(events);
                if !self.state.page_mode {
                    self.state.page_mode = true;
                    events.push(Event::Page(Page::Enter));
                }
                Step::Consumed(2)
            }
            0x53 => {
                self.flush_line(events);
                if self.state.page_mode {
                    self.state.page_mode = false;
                    events.push(Event::Page(Page::Standard));
                }
                Step::Consumed(2)
            }
            0x0c => {
                self.flush_line(events);
                if self.state.page_mode {
                    events.push(Event::Page(Page::Print));
                }
                Step::Consumed(2)
            }
            // ESC < returns the print head home, which means nothing here.
            0x3c => Step::Consumed(2),
            // ESC $ nL nH and ESC \ nL nH move the print position along the line.
            0x24 | 0x5c => {
                let (Some(low), Some(high)) = (input.get(2), input.get(3)) else {
                    return Step::Incomplete;
                };
                let value = u16::from(low) + u16::from(high) * 256;
                self.flush_line(events);
                events.push(Event::Position(if code == 0x24 {
                    Position::Absolute(value)
                } else {
                    Position::Relative(value as i16)
                }));
                Step::Consumed(4)
            }
            // ESC B n t sounds the buzzer.
            0x42 => {
                let (Some(times), Some(duration)) = (input.get(2), input.get(3)) else {
                    return Step::Incomplete;
                };
                self.flush_line(events);
                events.push(Event::Beep { times, duration });
                Step::Consumed(4)
            }
            // ESC c m n sets how the paper sensors and panel buttons work.
            0x63 => {
                if input.bytes.len() < 4 {
                    return Step::Incomplete;
                }
                Step::Consumed(4)
            }
            // ESC p m t1 t2 kicks the cash drawer.
            0x70 => {
                if input.bytes.len() < 5 {
                    return Step::Incomplete;
                }
                self.flush_line(events);
                events.push(Event::Pulse(Pulse {
                    pin: input.bytes[2] & 1,
                    on_ms: u16::from(input.bytes[3]) * 2,
                    off_ms: u16::from(input.bytes[4]) * 2,
                }));
                Step::Consumed(5)
            }
            // ESC W xL xH yL yH dxL dxH dyL dyH sets the page of page mode.
            0x57 => {
                if input.bytes.len() < 10 {
                    return Step::Incomplete;
                }
                let word =
                    |at: usize| u16::from(input.bytes[at]) + u16::from(input.bytes[at + 1]) * 256;
                let (width, height) = (word(6), word(8));
                if width == 0 || height == 0 {
                    return input.fixed_unknown(10, events);
                }
                self.flush_line(events);
                events.push(Event::Page(Page::Area {
                    x: word(2),
                    y: word(4),
                    width,
                    height,
                }));
                Step::Consumed(10)
            }
            0x44 => self.parse_tab_stops(input, events),
            // ESC * m nL nH d... is a picture in columns.
            0x2a => self.parse_column_image(input, events),
            // ESC & y c1 c2 d... defines characters, and has no length of its own.
            0x26 => self.parse_user_characters(input, events),
            // ESC ( fn pL pH data... has a length. Only the buzzer, ESC ( A, is read.
            0x28 => {
                let Some(function) = input.get(2) else {
                    return Step::Incomplete;
                };
                if function != b'A' {
                    return input.framed_unknown(3, 2, events);
                }
                match input.frame(3, 2) {
                    Frame::Need | Frame::Waiting => Step::Incomplete,
                    Frame::TooBig => input.unknown_run(events),
                    Frame::Whole(length) => {
                        self.flush_line(events);
                        events.push(Event::Beep {
                            times: 1,
                            duration: 0,
                        });
                        Step::Consumed(length)
                    }
                }
            }
            // The commands that take one byte.
            0x20 | 0x21 | 0x25 | 0x2b | 0x2d | 0x33 | 0x3d | 0x3f | 0x41 | 0x45 | 0x47 | 0x4a
            | 0x4d | 0x52 | 0x54 | 0x55 | 0x56 | 0x61 | 0x64 | 0x65 | 0x72 | 0x74 | 0x7b => {
                let Some(value) = input.get(2) else {
                    return Step::Incomplete;
                };
                self.one_byte_command(input, code, value, events)
            }
            _ => input.unknown_run(events),
        }
    }

    /// A value a command does not allow makes a printer ignore the command, and is reported as an
    /// unknown command here.
    fn one_byte_command(
        &mut self,
        input: &Input,
        code: u8,
        value: u8,
        events: &mut Vec<Event>,
    ) -> Step {
        match code {
            // ESC SP n adds space to the right of each character.
            0x20 => self.change_style(events, |style| style.spacing = value),
            // ESC ! n sets the font, bold, double height, double width and underline in one byte.
            0x21 => {
                let font = value & 1;
                self.change_style(events, |style| {
                    style.font = font;
                    style.bold = value & 0x08 != 0;
                    style.height = if value & 0x10 != 0 { 2 } else { 1 };
                    style.width = if value & 0x20 != 0 { 2 } else { 1 };
                    style.underline = u8::from(value & 0x80 != 0);
                });
                events.push(Event::Font(font));
            }
            // ESC % n turns the characters the program drew on and off.
            0x25 => {
                self.flush_line(events);
                self.state.user_characters = value & 1 == 1;
                events.push(Event::UserCharacters(self.state.user_characters));
            }
            0x2b => {
                self.flush_line(events);
                events.push(Event::LineSpacing(LineSpacing::ThreeSixtieths(value)));
            }
            // ESC - n turns the underline on, thin or thick, or off.
            0x2d => {
                let thickness = match value {
                    0 | 48 => 0,
                    1 | 49 => 1,
                    2 | 50 => 2,
                    _ => return input.fixed_unknown(3, events),
                };
                self.change_style(events, |style| style.underline = thickness);
            }
            0x33 => {
                self.flush_line(events);
                events.push(Event::LineSpacing(LineSpacing::Units(value)));
            }
            // ESC = n picks the device that the data goes to, and ESC U n the printing direction.
            0x3d | 0x55 => {}
            // ESC ? n takes back a character the program drew.
            0x3f => {
                if !(32..=126).contains(&value) {
                    return input.fixed_unknown(3, events);
                }
                self.flush_line(events);
                if self.state.user_defined.remove(&value) {
                    events.push(Event::DeleteCharacter(value));
                }
            }
            0x41 => {
                self.flush_line(events);
                events.push(Event::LineSpacing(LineSpacing::Sixtieths(value)));
            }
            0x45 => self.change_style(events, |style| style.bold = value & 1 == 1),
            0x47 => self.change_style(events, |style| style.double_strike = value & 1 == 1),
            0x4a => {
                self.flush_line(events);
                events.push(Event::FeedUnits(u16::from(value)));
            }
            // ESC M n picks font A to E, as 0 to 4 or as the digits '0' to '4'.
            0x4d => {
                let font = match value {
                    0..=4 => value,
                    48..=52 => value - 48,
                    _ => return input.fixed_unknown(3, events),
                };
                self.change_style(events, |style| style.font = font);
                events.push(Event::Font(font));
            }
            // ESC R n picks an international character set.
            0x52 => {
                if !matches!(value, 0..=17 | 66..=75) {
                    return input.fixed_unknown(3, events);
                }
                self.flush_line(events);
                self.state.international = value;
            }
            // ESC T n picks which way text runs in page mode.
            0x54 => {
                if !matches!(value, 0..=3 | 48..=51) {
                    return input.fixed_unknown(3, events);
                }
                self.flush_line(events);
                events.push(Event::Page(Page::Direction(value & 3)));
            }
            // ESC V n turns the characters a quarter turn.
            0x56 => {
                let rotated = match value {
                    0 | 48 => false,
                    1 | 49 => true,
                    _ => return input.fixed_unknown(3, events),
                };
                self.change_style(events, |style| style.rotated = rotated);
            }
            0x61 => {
                let align = match value {
                    0 | 48 => Align::Left,
                    1 | 49 => Align::Center,
                    2 | 50 => Align::Right,
                    _ => return input.fixed_unknown(3, events),
                };
                self.flush_line(events);
                self.state.align = align;
                events.push(Event::Align(align));
            }
            0x64 => {
                self.flush_line(events);
                events.push(Event::Feed(value));
            }
            0x65 => {
                self.flush_line(events);
                events.push(Event::ReverseFeed(value));
            }
            // ESC r n picks the first or the second colour.
            0x72 => {
                let color = match value {
                    0 | 48 => 0,
                    1 | 49 => 1,
                    _ => return input.fixed_unknown(3, events),
                };
                self.change_style(events, |style| style.color = color);
            }
            // ESC t n picks a page of the code table.
            0x74 => {
                self.flush_line(events);
                self.state.codepage = value;
            }
            // ESC { n turns the line upside down.
            _ => self.change_style(events, |style| style.upside_down = value & 1 == 1),
        }
        Step::Consumed(3)
    }

    /// `ESC D n1 ... nk NUL` sets the columns that the tab character jumps to. The columns go up,
    /// and a byte that does not ends the list and is read as text, as are the ones after the 32nd.
    fn parse_tab_stops(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some((stops, length)) = read_stops(input, MAX_TAB_STOPS) else {
            return Step::Incomplete;
        };
        self.flush_line(events);
        events.push(Event::TabStops(stops));
        Step::Consumed(length)
    }

    /// `ESC * m nL nH d...` is a picture in columns. The 8 dot modes print at a third of the
    /// density down the paper, and the single density modes at half the density across it, so the
    /// dots of those are repeated to fill what is printed.
    fn parse_column_image(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        if input.bytes.len() < 5 {
            return Step::Incomplete;
        }
        // The dots in a column, and how often a dot is repeated across and down.
        let (dots, across, down) = match input.bytes[2] {
            0 => (8, 2, 3),
            1 => (8, 1, 3),
            32 => (24, 2, 1),
            33 => (24, 1, 1),
            _ => return input.unknown_run(events),
        };
        let columns = usize::from(input.bytes[3]) + usize::from(input.bytes[4]) * 256;
        let length = 5 + columns * dots / 8;
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        self.flush_line(events);
        if let Some(bitmap) =
            Bitmap::from_columns(columns as u32, dots as u32, &input.bytes[5..length])
        {
            events.push(Event::Image(bitmap.scaled(across, down)));
        }
        Step::Consumed(length)
    }

    /// `ESC & y c1 c2` followed by, for each character from c1 to c2, its width x and y times x
    /// bytes of columns. The characters are drawn on the paper in place of the font's own.
    fn parse_user_characters(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        if input.bytes.len() < 5 {
            return Step::Incomplete;
        }
        let (height, first, last) = (input.bytes[2], input.bytes[3], input.bytes[4]);
        if height == 0 || !(32..=126).contains(&first) || !(first..=126).contains(&last) {
            return input.unknown_run(events);
        }
        let mut length = 5;
        for _ in first..=last {
            let Some(&width) = input.bytes.get(length) else {
                return Step::Incomplete;
            };
            length += 1 + usize::from(height) * usize::from(width);
            if length > MAX_COMMAND_BYTES {
                return input.unknown_run(events);
            }
        }
        if input.bytes.len() < length {
            return Step::Incomplete;
        }

        self.flush_line(events);
        // A printer cannot hold characters and a downloaded picture at the same time.
        self.state.downloaded = None;
        let mut index = 5;
        for code in first..=last {
            let width = input.bytes[index];
            let size = usize::from(height) * usize::from(width);
            events.push(Event::DefineCharacter(UserCharacter {
                code,
                width,
                column_bytes: height,
                data: input.bytes[index + 1..index + 1 + size].to_vec(),
            }));
            self.state.user_defined.insert(code);
            index += 1 + size;
        }
        Step::Consumed(length)
    }
}
