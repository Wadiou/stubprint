//! Pictures: raster pictures, the pictures a printer keeps in memory, and `GS ( L` graphics.

use super::memory::Area;
use super::{Decoder, Frame, Input, Step, MAX_COMMAND_BYTES, MAX_STORED_BYTES};
use crate::bitmap::Bitmap;
use crate::event::Event;

/// How a mode of `GS v 0`, `GS /` and `FS p` repeats dots: normal, double width, double height,
/// and both. The modes can be written as numbers or as the digits.
fn print_mode(mode: u8) -> Option<(u32, u32)> {
    match mode {
        0 | 48 => Some((1, 1)),
        1 | 49 => Some((2, 1)),
        2 | 50 => Some((1, 2)),
        3 | 51 => Some((2, 2)),
        _ => None,
    }
}

/// The magnification of `GS ( L`, which is 1 or 2.
fn magnification(value: u8) -> u32 {
    u32::from(value.clamp(1, 2))
}

impl Decoder {
    /// A picture is printed: the text before it is reported first.
    fn print_image(&mut self, bitmap: Bitmap, events: &mut Vec<Event>) {
        self.flush_line(events);
        events.push(Event::Image(bitmap));
    }

    /// Whether `bytes` more of pictures fit in what the printer remembers.
    fn room_for(&self, bytes: usize) -> bool {
        self.stored_bytes() + bytes <= MAX_STORED_BYTES
    }

    /// `GS v 0 m xL xH yL yH d...` is a picture in rows. `xL xH` is how many bytes are in a row.
    pub(super) fn parse_raster(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        if input.bytes.len() < 8 {
            return Step::Incomplete;
        }
        let Some((across, down)) = print_mode(input.bytes[3]) else {
            return input.unknown_run(events);
        };
        let row_bytes = usize::from(input.bytes[4]) + usize::from(input.bytes[5]) * 256;
        let height = usize::from(input.bytes[6]) + usize::from(input.bytes[7]) * 256;
        let length = 8 + row_bytes * height;
        if length > MAX_COMMAND_BYTES {
            return input.unknown_run(events);
        }
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        if let Some(bitmap) =
            Bitmap::from_packed(row_bytes, height as u32, input.bytes[8..length].to_vec())
        {
            self.print_image(bitmap.scaled(across, down), events);
        } else {
            self.flush_line(events);
        }
        Step::Consumed(length)
    }

    /// `GS * x y d...` defines a picture of x * 8 by y * 8 dots, in columns of y bytes, to be printed
    /// later with `GS /`.
    pub(super) fn parse_download_define(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let (Some(x), Some(y)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        let (x, y) = (usize::from(x), usize::from(y));
        if x == 0 || !(1..=48).contains(&y) || x * y > 1536 {
            return input.fixed_unknown(4, events);
        }
        let length = 4 + x * y * 8;
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        if !self.room_for(x * y * 8) {
            return input.fixed_unknown(length, events);
        }
        self.flush_line(events);
        self.state.downloaded =
            Bitmap::from_columns((x * 8) as u32, (y * 8) as u32, &input.bytes[4..length]);
        Step::Consumed(length)
    }

    /// `GS / m` prints the picture of `GS *`. A printer that has none ignores it.
    pub(super) fn parse_download_print(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(mode) = input.get(2) else {
            return Step::Incomplete;
        };
        let Some((across, down)) = print_mode(mode) else {
            return input.fixed_unknown(3, events);
        };
        if let Some(bitmap) = self.state.downloaded.clone() {
            self.print_image(bitmap.scaled(across, down), events);
        }
        Step::Consumed(3)
    }

    /// `FS q n [xL xH yL yH d...]...` defines n pictures that the printer keeps, each of
    /// (xL + xH * 256) * 8 by (yL + yH * 256) * 8 dots in columns, and forgets the older ones.
    pub(super) fn parse_nv_define(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(count) = input.get(2) else {
            return Step::Incomplete;
        };
        if count == 0 {
            return input.fixed_unknown(3, events);
        }
        let mut index = 3;
        let mut pictures: Vec<(usize, usize, usize)> = Vec::new();
        for _ in 0..count {
            if input.bytes.len() < index + 4 {
                return Step::Incomplete;
            }
            let x = usize::from(input.bytes[index]) + usize::from(input.bytes[index + 1]) * 256;
            let y = usize::from(input.bytes[index + 2]) + usize::from(input.bytes[index + 3]) * 256;
            if x == 0 || y == 0 || x > 1023 || y > 288 {
                return input.unknown_run(events);
            }
            pictures.push((index + 4, x, y));
            index += 4 + x * y * 8;
            if index > MAX_COMMAND_BYTES {
                return input.unknown_run(events);
            }
        }
        if input.bytes.len() < index {
            return Step::Incomplete;
        }
        let total: usize = pictures.iter().map(|(_, x, y)| x * y * 8).sum();
        if total > MAX_STORED_BYTES {
            return input.fixed_unknown(index, events);
        }

        self.flush_line(events);
        let bit_images = pictures
            .into_iter()
            .filter_map(|(start, x, y)| {
                Bitmap::from_columns(
                    (x * 8) as u32,
                    (y * 8) as u32,
                    &input.bytes[start..start + x * y * 8],
                )
            })
            .collect();
        self.memory.set_bit_images(bit_images);
        Step::Consumed(index)
    }

    /// `FS p n m` prints picture number n of `FS q`. A printer that has none ignores it.
    pub(super) fn parse_nv_print(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let (Some(number), Some(mode)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        let Some((across, down)) = print_mode(mode) else {
            return input.fixed_unknown(4, events);
        };
        if number >= 1 {
            if let Some(bitmap) = self.memory.bit_image(usize::from(number) - 1).cloned() {
                self.print_image(bitmap.scaled(across, down), events);
            }
        }
        Step::Consumed(4)
    }

    /// `GS ( L pL pH m fn ...` and `GS 8 L p1 p2 p3 p4 m fn ...` store graphics in the print buffer
    /// and print them, or keep them in the printer's memory under a key of two characters.
    pub(super) fn parse_graphics(
        &mut self,
        input: &Input,
        header: usize,
        width: usize,
        events: &mut Vec<Event>,
    ) -> Step {
        let length = match input.frame(3, width) {
            Frame::Need | Frame::Waiting => return Step::Incomplete,
            Frame::TooBig => return input.unknown_run(events),
            Frame::Whole(length) => length,
        };
        let body = &input.bytes[header..length];
        let known = match body {
            [48, function, params @ ..] => self.graphics_function(*function, params, events),
            _ => false,
        };
        if !known {
            events.push(Event::Unknown {
                offset: input.offset,
                bytes: input.bytes[..length].to_vec(),
            });
        }
        Step::Consumed(length)
    }

    /// Does what one function of `GS ( L` says, and tells whether it could.
    fn graphics_function(&mut self, function: u8, params: &[u8], events: &mut Vec<Event>) -> bool {
        // The functions 0 to 4 are other numbers for 48 to 52.
        let function = if function < 5 {
            function + 48
        } else {
            function
        };
        match function {
            // Store raster or column graphics in the print buffer, to be printed by function 50.
            112 | 113 => {
                let [_tone, across, down, color, x_low, x_high, y_low, y_high, data @ ..] = params
                else {
                    return false;
                };
                let (x, y) = (word(*x_low, *x_high), word(*y_low, *y_high));
                let Some(bitmap) = picture(function == 113, x, y, data) else {
                    return false;
                };
                let bitmap = bitmap.scaled(magnification(*across), magnification(*down));
                // The first colour starts a new graphic and the others are laid over it.
                match &mut self.state.graphics_buffer {
                    Some(buffer)
                        if *color != 49
                            && (buffer.width(), buffer.height())
                                == (bitmap.width(), bitmap.height()) =>
                    {
                        buffer.merge(&bitmap);
                    }
                    buffer => *buffer = Some(bitmap),
                }
                true
            }
            // Print the graphics in the print buffer.
            50 => {
                if let Some(bitmap) = self.state.graphics_buffer.take() {
                    self.print_image(bitmap, events);
                }
                true
            }
            // Define graphics under a key, in the memory that stays or the one that does not.
            67 | 68 | 83 | 84 => {
                let [_tone, key_1, key_2, colors, x_low, x_high, y_low, y_high, data @ ..] = params
                else {
                    return false;
                };
                let (x, y) = (word(*x_low, *x_high), word(*y_low, *y_high));
                let column = matches!(function, 68 | 84);
                let size = if column {
                    x as usize * (y as usize).div_ceil(8)
                } else {
                    (x as usize).div_ceil(8) * y as usize
                };
                let mut merged: Option<Bitmap> = None;
                let mut rest = data;
                for _ in 0..*colors {
                    let [_color, layer @ ..] = rest else {
                        return false;
                    };
                    if layer.len() < size {
                        return false;
                    }
                    let Some(bitmap) = picture(column, x, y, &layer[..size]) else {
                        return false;
                    };
                    match &mut merged {
                        Some(all) => all.merge(&bitmap),
                        none => *none = Some(bitmap),
                    }
                    rest = &layer[size..];
                }
                let Some(bitmap) = merged else {
                    return false;
                };
                if !self.room_for(bitmap.data().len()) {
                    return false;
                }
                self.memory
                    .define(area_of(function), (*key_1, *key_2), bitmap);
                true
            }
            // Print graphics from memory by their key, magnified.
            69 | 85 => {
                let [key_1, key_2, across, down] = params else {
                    return false;
                };
                let area = area_of(function);
                if let Some(bitmap) = self.memory.get(area, (*key_1, *key_2)).cloned() {
                    let bitmap = bitmap.scaled(magnification(*across), magnification(*down));
                    self.print_image(bitmap, events);
                }
                true
            }
            // Delete one graphic by its key, or all of them.
            66 | 82 => {
                let [key_1, key_2] = params else {
                    return false;
                };
                self.memory.remove(area_of(function), (*key_1, *key_2));
                true
            }
            65 | 81 => {
                self.memory.clear(area_of(function));
                true
            }
            // The density the graphics that follow are made for: 180 or 360 dots per inch.
            49 => {
                let dpi = match params {
                    [50, 50] => 180,
                    [51, 51] => 360,
                    _ => return false,
                };
                self.flush_line(events);
                events.push(Event::GraphicsDensity(dpi));
                true
            }
            // How much memory the printer has for graphics, and how much of it is left.
            48 | 51 | 52 => {
                if !params.is_empty() {
                    return false;
                }
                let area = if function == 52 {
                    Area::Download
                } else {
                    Area::Nv
                };
                let answer = self.memory.size_answer(area, function != 48);
                self.reply(&answer);
                true
            }
            // The keys of the graphics in memory, sent in blocks that the program asks for.
            64 | 80 => {
                if params != b"KC" {
                    return false;
                }
                let answer = self.memory.list(area_of(function));
                self.reply(&answer);
                true
            }
            _ => false,
        }
    }
}

/// The functions of `GS ( L` below 80 are about the memory that stays, and from 80 up about the
/// one that does not.
fn area_of(function: u8) -> Area {
    if function < 80 {
        Area::Nv
    } else {
        Area::Download
    }
}

fn word(low: u8, high: u8) -> u32 {
    u32::from(low) + u32::from(high) * 256
}

/// Graphics of x by y dots from rows or from columns.
fn picture(column: bool, x: u32, y: u32, data: &[u8]) -> Option<Bitmap> {
    if column {
        Bitmap::from_columns(x, y, data)
    } else {
        Bitmap::from_rows(x, y, data.to_vec())
    }
}
