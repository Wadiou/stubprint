//! A receipt as a picture of the paper, one dot for each dot the printer prints.

use stubprint_core::{Align, Barcode, Bitmap, TextPosition};

use crate::blocks::{Block, Document, Glyph, Line, Part};
use crate::canvas::Canvas;
use crate::codes::{self, Modules};
use crate::fonts::{self, Cell};
use crate::Options;

/// White above the first line and below the last.
const EDGE: u32 = 8;
/// Where the baseline is in a line, as a share of its height, which leaves room for the tails of
/// letters like g and y.
const BASELINE: f32 = 0.78;
/// How tall the paper can get, in dots: 12 metres at 203 dots to the inch. A stream that feeds more
/// is cut short, so a program cannot make the drawing take all the memory.
const MAX_HEIGHT: u32 = 96_000;
/// The most pixels a picture can have after it is made larger.
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
/// How many dots a module of Aztec and Data Matrix codes takes, which their settings do not say.
const SQUARE_MODULE: u32 = 4;

/// Where something that is `width` dots wide starts, on a line that is `room` dots wide.
fn start(align: Align, room: u32, width: u32) -> i64 {
    match align {
        Align::Left => 0,
        Align::Center => i64::from(room.saturating_sub(width)) / 2,
        Align::Right => i64::from(room.saturating_sub(width)),
    }
}

struct Placed<'a> {
    character: char,
    /// From the start of the print area.
    x: i64,
    /// How many cells wide, and so how many times the cell width.
    cells: usize,
    style: &'a stubprint_core::Style,
}

/// Where each character of a line goes, and how far along the line the characters reach.
fn place<'a>(line: &'a Line, options: &Options) -> (Vec<Placed<'a>>, u32, bool) {
    let mut placed = Vec::new();
    let mut cursor = 0i64;
    let mut reach = 0i64;
    let mut moved = false;
    for part in &line.parts {
        match part {
            Part::Text { text, style } => {
                let cell = i64::from(options.cell_width(style.font) * u32::from(style.width));
                for character in text.chars() {
                    let cells = fonts::columns(character);
                    if cells == 0 {
                        continue;
                    }
                    placed.push(Placed {
                        character,
                        x: cursor,
                        cells,
                        style,
                    });
                    cursor += cell * cells as i64 + i64::from(style.spacing);
                    reach = reach.max(cursor);
                }
            }
            Part::Tab => {
                let cell = i64::from(options.cell_width(0));
                if let Some(stop) = line
                    .tab_stops
                    .iter()
                    .map(|stop| i64::from(*stop) * cell)
                    .find(|stop| *stop > cursor)
                {
                    cursor = stop;
                    reach = reach.max(cursor);
                }
            }
            Part::Move { absolute, dots } => {
                moved = true;
                cursor = if *absolute {
                    i64::from(*dots)
                } else {
                    cursor + i64::from(*dots)
                };
                cursor = cursor.max(0);
            }
        }
    }
    (placed, reach.max(0) as u32, moved)
}

fn draw_user_glyph(
    strip: &mut Canvas,
    glyph: &Glyph,
    x: i64,
    top: i64,
    style: &stubprint_core::Style,
) {
    let (across, down) = (i64::from(style.width), i64::from(style.height));
    for column in 0..glyph.width {
        for row in 0..glyph.height {
            if glyph.is_black(column, row) {
                strip.rect(
                    x + column as i64 * across,
                    top + row as i64 * down,
                    across,
                    down,
                    true,
                );
            }
        }
    }
}

/// How tall a line is, in dots: as its tallest piece of text.
fn line_height(line: &Line, options: &Options) -> u32 {
    line.parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { style, .. } => {
                Some(options.cell_height(style.font) * u32::from(style.height))
            }
            _ => None,
        })
        .max()
        .unwrap_or_else(|| options.cell_height(0))
}

/// One line as a strip as wide as the paper.
fn line_strip(line: &Line, document: &Document, options: &Options) -> Canvas {
    let (placed, reach, moved) = place(line, options);
    let height = line_height(line, options);
    let mut strip = Canvas::new(options.width, height);
    let origin = i64::from(line.left)
        + if moved {
            0
        } else {
            start(line.align, line.area, reach)
        };
    let baseline = (height as f32 * BASELINE).round() as i64;

    for item in &placed {
        let style = item.style;
        let width = options.cell_width(style.font) * u32::from(style.width);
        let cell_height = options.cell_height(style.font) * u32::from(style.height);
        let x = origin + item.x;
        let span = i64::from(width) * item.cells as i64;
        if style.invert {
            strip.rect(x, 0, span, i64::from(height), true);
        }
        let black = !style.invert;
        if let Some(glyph) = document.glyphs.get(&item.character) {
            let top = baseline - (cell_height as f32 * BASELINE).round() as i64;
            if black {
                draw_user_glyph(&mut strip, glyph, x, top, style);
            }
        } else if !(0xe000..=0xe1ff).contains(&u32::from(item.character)) {
            let cell = Cell {
                x,
                baseline,
                width,
                height: cell_height,
            };
            fonts::draw(&mut strip, item.character, cell, style.bold, black);
            if style.double_strike {
                fonts::draw(
                    &mut strip,
                    item.character,
                    Cell { x: x + 1, ..cell },
                    style.bold,
                    black,
                );
            }
        }
        if style.underline > 0 {
            strip.rect(x, baseline + 2, span, i64::from(style.underline), black);
        }
        if style.overline {
            strip.rect(x, 0, span, 1, black);
        }
    }
    if line
        .parts
        .iter()
        .any(|part| matches!(part, Part::Text { style, .. } if style.upside_down))
    {
        strip = strip.turned_around();
    }
    strip
}

/// The dots of a picture as black on white.
fn bitmap_strip(bitmap: &Bitmap) -> Canvas {
    let mut strip = Canvas::new(bitmap.width(), bitmap.height());
    for y in 0..bitmap.height() {
        for x in 0..bitmap.width() {
            if bitmap.is_black(x, y) {
                strip.rect(i64::from(x), i64::from(y), 1, 1, true);
            }
        }
    }
    strip
}

/// A code's modules, each made `across` dots wide and `down` dots high.
fn modules_strip(modules: &Modules, across: u32, down: u32) -> Canvas {
    let mut strip = Canvas::new(modules.width * across, modules.height * down);
    for y in 0..modules.height {
        for x in 0..modules.width {
            if modules.is_dark(x, y) {
                strip.rect(
                    i64::from(x * across),
                    i64::from(y * down),
                    i64::from(across),
                    i64::from(down),
                    true,
                );
            }
        }
    }
    strip
}

/// Characters one after the other from `x`, as the readable text of a barcode is.
fn write(strip: &mut Canvas, text: &str, x: i64, baseline: i64, options: &Options, font: u8) {
    let (width, height) = (options.cell_width(font), options.cell_height(font));
    let mut cursor = x;
    for character in text.chars() {
        let cells = fonts::columns(character);
        if cells == 0 {
            continue;
        }
        let cell = Cell {
            x: cursor,
            baseline,
            width,
            height,
        };
        fonts::draw(strip, character, cell, false, true);
        cursor += i64::from(width) * cells as i64;
    }
}

fn text_width(text: &str, options: &Options, font: u8) -> u32 {
    text.chars().map(|c| fonts::columns(c) as u32).sum::<u32>() * options.cell_width(font)
}

/// A box that says what could not be drawn, where the code would be.
fn placeholder(label: &str, options: &Options) -> Canvas {
    let label: String = label
        .chars()
        .take(usize::from(options.columns[0]).saturating_sub(2))
        .collect();
    let width = (text_width(&label, options, 0) + 16).min(options.width);
    let height = options.cell_height(0) + 16;
    let mut strip = Canvas::new(width, height);
    strip.rect(0, 0, i64::from(width), 1, true);
    strip.rect(0, i64::from(height) - 1, i64::from(width), 1, true);
    strip.rect(0, 0, 1, i64::from(height), true);
    strip.rect(i64::from(width) - 1, 0, 1, i64::from(height), true);
    let baseline = 8 + (options.cell_height(0) as f32 * BASELINE).round() as i64;
    write(&mut strip, &label, 8, baseline, options, 0);
    strip
}

fn barcode_strip(barcode: &Barcode, options: &Options) -> Canvas {
    let modules = match codes::barcode(barcode) {
        Ok(modules) => modules,
        Err(reason) => {
            return placeholder(&format!("{}: {reason}", barcode.symbology.name()), options)
        }
    };
    let (across, bars) = (
        u32::from(barcode.width.max(1)),
        u32::from(barcode.height.max(1)),
    );
    let drawing = modules_strip(&modules, across, bars);
    let show_above = matches!(barcode.text, TextPosition::Above | TextPosition::Both);
    let show_below = matches!(barcode.text, TextPosition::Below | TextPosition::Both);
    let text_height = options.cell_height(barcode.font);
    let label = text_width(&barcode.data, options, barcode.font);
    let width = drawing.width.max(label);
    let height = drawing.height
        + if show_above { text_height } else { 0 }
        + if show_below { text_height } else { 0 };
    let mut strip = Canvas::new(width, height);
    let top = if show_above { text_height } else { 0 };
    strip.lay(
        &drawing,
        start(Align::Center, width, drawing.width),
        i64::from(top),
    );
    let baseline = (text_height as f32 * BASELINE).round() as i64;
    let x = start(Align::Center, width, label);
    if show_above {
        write(
            &mut strip,
            &barcode.data,
            x,
            baseline,
            options,
            barcode.font,
        );
    }
    if show_below {
        write(
            &mut strip,
            &barcode.data,
            x,
            i64::from(top + drawing.height) + baseline,
            options,
            barcode.font,
        );
    }
    strip
}

/// What a code block looks like: the code, or a box that says why it is not drawn.
fn code_strip(block: &Block, options: &Options) -> Option<Canvas> {
    let drawn = |result: Result<Modules, String>, across: u32, down: u32, name: &str| match result {
        Ok(modules) => modules_strip(&modules, across, down),
        Err(reason) => placeholder(&format!("{name}: {reason}"), options),
    };
    Some(match block {
        Block::Barcode(barcode, _) => barcode_strip(barcode, options),
        Block::Qr(qr, _) => {
            let size = u32::from(qr.size).clamp(1, 16);
            drawn(codes::qr(qr), size, size, "qr")
        }
        Block::Pdf417(code, _) => {
            let across = u32::from(code.width).clamp(1, 16);
            // The encoder makes each row 4 dots high, and the printer makes it `height` modules.
            let down = (across * u32::from(code.height).max(1) / 4).max(1);
            drawn(codes::pdf417(code), across, down, "pdf417")
        }
        Block::Symbol(symbol, _) => drawn(
            codes::symbol(symbol),
            SQUARE_MODULE,
            SQUARE_MODULE,
            symbol.kind.name(),
        ),
        _ => return None,
    })
}

fn align_of(block: &Block) -> Align {
    match block {
        Block::Image(_, align)
        | Block::Barcode(_, align)
        | Block::Qr(_, align)
        | Block::Pdf417(_, align)
        | Block::Symbol(_, align) => *align,
        _ => Align::Left,
    }
}

/// How tall the paper is, in dots, which is as tall as [`paint`] makes the picture at scale 1. It
/// works out the places and not the dots, so it is much quicker than drawing.
pub fn height(document: &Document, options: &Options) -> u32 {
    let mut y = EDGE;
    for block in &document.blocks {
        if y > MAX_HEIGHT {
            break;
        }
        match block {
            Block::Line(line) => {
                let height = line_height(line, options);
                y += if line.ended {
                    line.pitch.max(height)
                } else {
                    height
                };
            }
            Block::Feed(dots) => y += dots,
            Block::Cut => {}
            Block::Image(bitmap, _) => y += bitmap.height(),
            other => {
                if let Some(strip) = code_strip(other, options) {
                    y += strip.height;
                }
            }
        }
    }
    y.min(MAX_HEIGHT + EDGE) + EDGE
}

/// The picture of what the paper got, as wide as the paper.
pub fn paint(document: &Document, options: &Options) -> Canvas {
    let mut canvas = Canvas::new(options.width, 0);
    let mut y = EDGE;
    for block in &document.blocks {
        if y > MAX_HEIGHT {
            break;
        }
        match block {
            Block::Line(line) => {
                let strip = line_strip(line, document, options);
                canvas.grow_to(y + strip.height);
                canvas.lay(&strip, 0, i64::from(y));
                y += if line.ended {
                    line.pitch.max(strip.height)
                } else {
                    strip.height
                };
            }
            Block::Feed(dots) => y += dots,
            Block::Cut => {}
            other => {
                let strip = match other {
                    Block::Image(bitmap, _) => Some(bitmap_strip(bitmap)),
                    _ => code_strip(other, options),
                };
                if let Some(strip) = strip {
                    canvas.grow_to(y + strip.height);
                    let x = start(align_of(other), options.width, strip.width);
                    canvas.lay(&strip, x, i64::from(y));
                    y += strip.height;
                }
            }
        }
    }
    canvas.grow_to(y.min(MAX_HEIGHT + EDGE) + EDGE);
    let mut scale = options.scale.clamp(1, 4);
    while scale > 1
        && u64::from(canvas.width) * u64::from(canvas.height) * u64::from(scale * scale)
            > MAX_PIXELS
    {
        scale -= 1;
    }
    canvas.enlarged(scale)
}

#[cfg(test)]
mod tests {
    use stubprint_core::Printer;

    use super::*;
    use crate::{picture, Options};

    const CUT: [u8; 4] = [0x1d, b'V', 0x42, 0];

    fn draw_with(bytes: &[u8], options: &Options) -> Canvas {
        let mut printer = Printer::new();
        let mut all = printer.push(&[bytes, &CUT].concat());
        all.extend(printer.finish());
        picture(&all[0], options)
    }

    fn draw(bytes: &[u8]) -> Canvas {
        draw_with(bytes, &Options::default())
    }

    fn black(canvas: &Canvas, x: i64, y: i64) -> bool {
        canvas.get(x, y).is_some_and(|value| value < 128)
    }

    /// GS v 0: a picture 1 byte wide and as tall as the rows given, all black.
    fn block(rows: u8) -> Vec<u8> {
        let mut bytes = vec![0x1d, b'v', b'0', 0, 1, 0, rows, 0];
        bytes.extend(std::iter::repeat_n(0xff, usize::from(rows)));
        bytes
    }

    #[test]
    fn the_height_is_the_height_of_the_picture() {
        let options = Options::default();
        for bytes in [
            b"one\ntwo\n".as_slice(),
            &[
                0x1b, b'!', 0x30, b'B', b'i', b'g', b'\n', 0x1b, b'd', 5, b'x',
            ],
            &[0x1d, b'k', 4, b'1', b'2', b'3', 0, 0x1b, b'J', 40],
            &[],
        ] {
            let mut printer = Printer::new();
            let mut all = printer.push(&[bytes, b"\nend", &CUT].concat());
            all.extend(printer.finish());
            let document = crate::blocks::document(&all[0], &options);
            assert_eq!(
                height(&document, &options),
                paint(&document, &options).height,
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn a_picture_goes_where_the_alignment_says() {
        // 8 dots wide, 2 high, below the 8 dots of white at the top.
        let left = draw(&block(2));
        let centre = draw(&[&b"\x1ba\x01"[..], &block(2)].concat());
        let right = draw(&[&b"\x1ba\x02"[..], &block(2)].concat());
        for (canvas, from) in [(&left, 0), (&centre, 284), (&right, 568)] {
            for y in [8, 9] {
                assert!(!black(canvas, from - 1, y));
                assert!(black(canvas, from, y) && black(canvas, from + 7, y));
                assert!(!black(canvas, from + 8, y));
            }
            assert!(!black(canvas, from, 7) && !black(canvas, from, 10));
        }
    }

    #[test]
    fn a_feed_leaves_white_paper_of_the_height_it_asks_for() {
        // ESC J 20 between two pictures: the second starts 2 + 20 dots after the first.
        let canvas = draw(&[block(2), b"\x1bJ\x14".to_vec(), block(2)].concat());
        assert!(black(&canvas, 0, 9) && !black(&canvas, 0, 10));
        assert!(!black(&canvas, 0, 29) && black(&canvas, 0, 30));
        assert_eq!(canvas.height, 8 + 2 + 20 + 2 + 8);
    }

    #[test]
    fn a_line_feed_moves_the_paper_by_the_line_pitch() {
        // Two lines of 24 dots with the default pitch of a sixth of an inch, 33 dots.
        let canvas = draw(b"x\nx\n");
        assert_eq!(canvas.height, 8 + 33 + 33 + 8);
    }

    #[test]
    fn text_is_underlined_inverted_and_turned_over_as_asked() {
        // The baseline of a 24 dot line is at 19, and the underline is 2 dots below it.
        let underlined = draw(b"\x1b-\x01A\n");
        assert!((0..12).all(|x| black(&underlined, x, 8 + 21)));
        assert!(!black(&underlined, 12, 8 + 21));

        // An inverted cell is black, and its character is white inside.
        let inverted = draw(b"\x1dB\x01A\n");
        assert!(black(&inverted, 0, 8) && black(&inverted, 11, 8 + 23));
        assert!(!black(&inverted, 12, 8));
        let white = (0..12)
            .flat_map(|x| (8..32).map(move |y| (x, y)))
            .filter(|(x, y)| !black(&inverted, *x, *y))
            .count();
        assert!(white > 20, "{white}");

        // Upside down is the same line with every dot turned half a turn about the line.
        let normal = draw(b"Ab\n");
        let turned = draw(b"\x1b{\x01Ab\n");
        for y in 8..32 {
            for x in 0..576 {
                assert_eq!(
                    black(&turned, x, y),
                    black(&normal, 575 - x, 8 + 31 - y),
                    "{x},{y}"
                );
            }
        }
    }

    #[test]
    fn text_in_a_bigger_size_is_drawn_bigger() {
        let single = draw(b"I\n");
        let double = draw(b"\x1d!\x11I\n");
        let ink = |canvas: &Canvas| canvas.pixels.iter().filter(|p| **p < 128).count();
        assert!(
            ink(&double) > 3 * ink(&single),
            "{} {}",
            ink(&double),
            ink(&single)
        );
        // The double height line is 48 dots tall, which is more than the pitch.
        assert_eq!(double.height, 8 + 48 + 8);
    }

    #[test]
    fn a_barcode_has_the_modules_the_event_says_and_a_code_that_fails_is_a_box() {
        // EAN-13 of 12 digits, 2 dots a module, 40 dots high, centred, without a text.
        let canvas = draw(b"\x1ba\x01\x1dw\x02\x1dh\x28\x1dH\x00\x1dkC\x0c400638133393");
        // 95 modules of 2 dots are 190 dots, which start at (576 - 190) / 2 = 193.
        assert!(black(&canvas, 193, 8) && black(&canvas, 194, 8));
        assert!(!black(&canvas, 192, 8) && !black(&canvas, 195, 8));
        assert!(black(&canvas, 193, 47) && !black(&canvas, 193, 48));
        assert!(black(&canvas, 193 + 188, 8) && black(&canvas, 193 + 189, 8));
        assert!(!black(&canvas, 193 + 190, 8));

        // A code with the wrong number of digits cannot be drawn: a box of the text says so.
        let bad = draw(b"\x1dkC\x02\x31\x32");
        assert!(black(&bad, 0, 8) && black(&bad, 0, 8 + 20));
        assert!(bad.height > 30);
    }

    #[test]
    fn the_text_of_a_barcode_goes_above_or_below_the_bars() {
        let ink_rows = |canvas: &Canvas, from: i64, to: i64| {
            (from..to)
                .filter(|y| (0..576).any(|x| black(canvas, x, *y)))
                .count()
        };
        let below = draw(b"\x1dh\x28\x1dH\x02\x1dkC\x0c400638133393");
        let above = draw(b"\x1dh\x28\x1dH\x01\x1dkC\x0c400638133393");
        assert_eq!(below.height, above.height);
        // The bars are 40 dots high and the text 24: the text is under them or over them.
        assert!(ink_rows(&below, 8 + 40, 8 + 64) > 5);
        assert_eq!(ink_rows(&below, 8, 8 + 40), 40);
        assert!(ink_rows(&above, 8, 8 + 24) > 5);
        assert_eq!(ink_rows(&above, 8 + 24, 8 + 64), 40);
    }

    #[test]
    fn a_picture_can_be_made_larger_and_a_huge_feed_is_cut_short() {
        let options = Options {
            scale: 2,
            ..Options::default()
        };
        let canvas = draw_with(&block(2), &options);
        assert_eq!((canvas.width, canvas.height), (1152, (8 + 2 + 8) * 2));
        assert!(black(&canvas, 0, 16) && black(&canvas, 1, 17) && !black(&canvas, 16, 16));

        // Thousands of feeds of the largest size stop at the limit.
        let feeds = b"\x1bJ\xff".repeat(2000);
        let big = draw(&[b"x\n".to_vec(), feeds].concat());
        assert!(big.height <= MAX_HEIGHT + 2 * EDGE + 255, "{}", big.height);
    }

    #[test]
    fn a_character_the_program_drew_is_drawn_from_its_dots() {
        // A 2 column character whose first column has the top dot and whose second has the last.
        let bytes = [
            &b"\x1b&\x03\x41\x41\x02"[..],
            &[0x80, 0, 0, 0, 0, 1],
            b"\x1b%\x01A\n",
        ]
        .concat();
        let canvas = draw(&bytes);
        let top = 8 + 19 - 19; // the glyph's top is at the top of the 24 dot cell
        assert!(black(&canvas, 0, i64::from(top)));
        assert!(black(&canvas, 1, i64::from(top) + 23));
        assert!(!black(&canvas, 1, i64::from(top)));
    }
}
