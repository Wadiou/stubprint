//! The commands that start with `GS`.

use super::super::machine::Machine;
use super::super::reader::{Input, Step};
use super::EscPos;
use crate::event::{CutMode, Event, Page, Position, TextPosition};

impl EscPos {
    pub(super) fn gs(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(code), Some(third)) = (input.get(1), input.get(2)) else {
            return Step::Incomplete;
        };
        match code {
            // GS V m cuts the paper. The forms with m of 65 and up also take the length n to feed.
            0x56 => {
                let (mode, length) = match third {
                    0 | 48 => (CutMode::Full, 3),
                    1 | 49 => (CutMode::Partial, 3),
                    65 | 97 | 103 => (CutMode::Full, 4),
                    66 | 98 | 104 => (CutMode::Partial, 4),
                    _ => return input.unknown_run(events),
                };
                if input.bytes.len() < length {
                    return Step::Incomplete;
                }
                machine.flush_line(events);
                events.push(Event::Cut(mode));
                Step::Consumed(length)
            }
            // GS h n, GS w n and GS H n set up the next barcode. GS f n picks the font for its text.
            0x68 => {
                if third == 0 {
                    return input.fixed_unknown(3, events);
                }
                self.barcode.height = third;
                Step::Consumed(3)
            }
            0x77 => {
                if third == 0 {
                    return input.fixed_unknown(3, events);
                }
                self.barcode.width = third;
                Step::Consumed(3)
            }
            0x48 => {
                self.barcode.text = match third {
                    0 | 48 => TextPosition::None,
                    1 | 49 => TextPosition::Above,
                    2 | 50 => TextPosition::Below,
                    3 | 51 => TextPosition::Both,
                    _ => return input.fixed_unknown(3, events),
                };
                Step::Consumed(3)
            }
            0x66 => {
                self.barcode.font = match third {
                    0 | 48 => 0,
                    1 | 49 => 1,
                    _ => return input.fixed_unknown(3, events),
                };
                Step::Consumed(3)
            }
            // GS ! n sets the character size, from 1 to 8 times in each direction. A value that has
            // other bits set is outside what the command allows, and a printer ignores it.
            0x21 => {
                if third & 0x88 != 0 {
                    return input.fixed_unknown(3, events);
                }
                let (width, height) = ((third >> 4) + 1, (third & 0x07) + 1);
                machine.change_style(events, |style| {
                    style.width = width;
                    style.height = height;
                });
                Step::Consumed(3)
            }
            // GS B n turns white on black on or off, and GS b n smooths big characters.
            0x42 => {
                machine.change_style(events, |style| style.invert = third & 1 == 1);
                Step::Consumed(3)
            }
            0x62 => {
                machine.change_style(events, |style| style.smooth = third & 1 == 1);
                Step::Consumed(3)
            }
            // GS | n sets how dark the print is.
            0x7c => {
                let Some(density) = density(third) else {
                    return input.fixed_unknown(3, events);
                };
                machine.flush_line(events);
                events.push(Event::Density(density));
                Step::Consumed(3)
            }
            // GS L nL nH, GS W nL nH and GS P x y set the margin, the print area and the motion units.
            0x4c | 0x57 | 0x50 | 0x24 | 0x5c => {
                let Some(fourth) = input.get(3) else {
                    return Step::Incomplete;
                };
                let value = u16::from(third) + u16::from(fourth) * 256;
                match code {
                    0x4c => {
                        machine.flush_line(events);
                        events.push(Event::Margin(value));
                    }
                    0x57 => {
                        machine.flush_line(events);
                        events.push(Event::PrintWidth(value));
                    }
                    0x50 => {
                        machine.flush_line(events);
                        events.push(Event::MotionUnits {
                            x: third,
                            y: fourth,
                        });
                    }
                    // GS $ and GS \ move in page mode, and a printer in standard mode ignores them.
                    _ => {
                        if machine.state.page_mode {
                            machine.flush_line(events);
                            events.push(Event::Page(Page::Vertical(if code == 0x24 {
                                Position::Absolute(value)
                            } else {
                                Position::Relative(value as i16)
                            })));
                        }
                    }
                }
                Step::Consumed(4)
            }
            // GS a n asks the printer to report its status by itself, and GS r n and GS I n ask for it.
            0x61 => {
                machine.flush_line(events);
                self.automatic_status = third;
                if third != 0 {
                    let status = machine.status.automatic();
                    machine.reply(&status);
                }
                Step::Consumed(3)
            }
            0x72 => {
                let Some(answer) = machine.status.transmit(third) else {
                    return input.fixed_unknown(3, events);
                };
                machine.reply(&[answer]);
                Step::Consumed(3)
            }
            0x49 => {
                self.printer_id(machine, third);
                Step::Consumed(3)
            }
            0x6b => self.parse_barcode(machine, input, events),
            0x28 => self.parse_gs_function(machine, input, third, events),
            // GS 8 L is the graphics command with a length of four bytes.
            0x38 if third == b'L' => machine.parse_graphics(input, 7, 4, events),
            0x38 => input.framed_unknown(3, 4, events),
            0x76 if third == 0x30 => machine.parse_raster(input, events),
            0x2a => machine.parse_download_define(input, events),
            0x2f => machine.parse_download_print(input, events),
            _ => input.unknown_run(events),
        }
    }

    /// `GS ( function pL pH data...`. The functions that only set up the printer's hardware are
    /// read and left alone.
    fn parse_gs_function(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        function: u8,
        events: &mut Vec<Event>,
    ) -> Step {
        match function {
            b'k' => machine.parse_symbol(input, events),
            b'L' => machine.parse_graphics(input, 5, 2, events),
            // GS ( H asks the printer to send back a code that identifies the request.
            b'H' => self.parse_response_request(machine, input, events),
            // Test print, macros, user settings, print control and character colours.
            b'A' | b'C' | b'D' | b'E' | b'K' | b'M' | b'N' => input.framed_ignored(3, 2, events),
            _ => input.framed_unknown(3, 2, events),
        }
    }

    /// `GS ( H pL pH fn m d1 d2 d3 d4`: with fn of 48 the printer sends back 0x37, 0x22, the four
    /// bytes it was given, and a NUL.
    fn parse_response_request(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        use super::super::reader::Frame;
        match input.frame(3, 2) {
            Frame::Need | Frame::Waiting => Step::Incomplete,
            Frame::TooBig => input.unknown_run(events),
            Frame::Whole(length) => {
                let body = &input.bytes[5..length];
                if body.len() == 6 && body[0] == 48 {
                    let mut answer = vec![0x37, 0x22];
                    answer.extend_from_slice(&body[2..6]);
                    answer.push(0);
                    machine.reply(&answer);
                }
                Step::Consumed(length)
            }
        }
    }

    /// `GS I n`: the model, the type and the version as one byte each, or the maker, the model name,
    /// the version and the serial number as text, which starts with `_` and ends with a NUL.
    fn printer_id(&mut self, machine: &mut Machine, n: u8) {
        let identity = &machine.profile.identity;
        let text = |value: &str| -> Vec<u8> {
            let mut answer = vec![0x5f];
            answer.extend_from_slice(value.as_bytes());
            answer.push(0);
            answer
        };
        let answer = match n {
            1 | 49 => vec![identity.model_id],
            2 | 50 => vec![identity.type_id],
            3 | 51 => vec![0x01],
            65 => text(&identity.firmware),
            66 => text(&identity.manufacturer),
            67 => text(&identity.model),
            68 => text(&identity.serial),
            _ => return,
        };
        machine.reply(&answer);
    }
}

/// `GS | n`: 0 to 4 are -50% to the usual density in steps of 12.5%, and 5 to 8 go on up, 5 being
/// +12.5% and 8 +50%, though the numbers an encoder writes for them are not in that order.
fn density(n: u8) -> Option<i16> {
    match n {
        0..=4 => Some(-500 + i16::from(n) * 125),
        5..=8 => Some((i16::from(n) - 4) * 125),
        _ => None,
    }
}
