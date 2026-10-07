//! A receipt as lines of text.

use receipt::Align;

use crate::blocks::{Block, Document, Line, Part, DPI};
use crate::Options;

/// How many columns a character takes: the East Asian wide ones take two.
fn columns_of(character: char) -> usize {
    match u32::from(character) {
        0x1100..=0x115f
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6 => 2,
        // Marks that join the character before them take none.
        0x0300..=0x036f | 0x064b..=0x065f | 0x200b..=0x200f => 0,
        _ => 1,
    }
}

/// Puts a tag such as `[qr: hello]` on a line of its own, where the picture would be.
fn tag(text: String, align: Align, options: &Options) -> String {
    let room = usize::from(options.columns[0]);
    let width = text.chars().count();
    let shift = match align {
        Align::Left => 0,
        Align::Center => room.saturating_sub(width) / 2,
        Align::Right => room.saturating_sub(width),
    };
    format!("{}{text}", " ".repeat(shift))
}

fn line(line: &Line, options: &Options) -> String {
    let cell = options.cell_width(0) as usize;
    let mut cells: Vec<char> = Vec::new();
    let mut cursor = 0usize;
    let mut moved = false;
    for part in &line.parts {
        match part {
            Part::Text { text, style } => {
                for character in text.chars() {
                    let columns = columns_of(character) * usize::from(style.width);
                    if columns == 0 {
                        continue;
                    }
                    if cells.len() < cursor + columns {
                        cells.resize(cursor + columns, ' ');
                    }
                    cells[cursor] = character;
                    cursor += columns;
                }
            }
            Part::Tab => {
                if let Some(stop) = line
                    .tab_stops
                    .iter()
                    .map(|s| usize::from(*s))
                    .find(|s| *s > cursor)
                {
                    cursor = stop;
                }
            }
            Part::Move { absolute, dots } => {
                moved = true;
                let columns = *dots / cell as i32;
                cursor = if *absolute {
                    columns.max(0) as usize
                } else {
                    (cursor as i32 + columns).max(0) as usize
                };
            }
        }
    }
    let room = line.area as usize / cell;
    let shift = if moved {
        0
    } else {
        match line.align {
            Align::Left => 0,
            Align::Center => room.saturating_sub(cells.len()) / 2,
            Align::Right => room.saturating_sub(cells.len()),
        }
    };
    let text: String =
        " ".repeat(line.left as usize / cell + shift) + &cells.iter().collect::<String>();
    text.trim_end().to_string()
}

/// The lines of the paper, where a picture or a code is a tag that says what it is.
pub fn text(document: &Document, options: &Options) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut blocks = document.blocks.iter().peekable();
    while let Some(block) = blocks.next() {
        match block {
            Block::Line(item) => lines.push(line(item, options)),
            Block::Feed(dots) => {
                let blank = (*dots as f32 / (DPI / 6) as f32).round() as usize;
                lines.extend(std::iter::repeat_n(String::new(), blank));
            }
            Block::Image(bitmap, align) => {
                // The bands of one picture are printed one after the other.
                let (width, mut height) = (bitmap.width(), bitmap.height());
                while let Some(Block::Image(next, _)) = blocks.peek() {
                    height += next.height();
                    blocks.next();
                }
                lines.push(tag(format!("[image {width}x{height}]"), *align, options));
            }
            Block::Barcode(barcode, align) => lines.push(tag(
                format!("[barcode {}: {}]", barcode.symbology.name(), barcode.data),
                *align,
                options,
            )),
            Block::Qr(qr, align) => lines.push(tag(
                format!("[qr {}: {}]", qr.level.name(), qr.data),
                *align,
                options,
            )),
            Block::Pdf417(code, align) => {
                lines.push(tag(format!("[pdf417: {}]", code.data), *align, options));
            }
            Block::Symbol(symbol, align) => {
                lines.push(tag(format!("[{}]", symbol.kind.name()), *align, options));
            }
            Block::Cut => {}
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use receipt::Printer;

    use crate::{text_of, Options};

    fn text(bytes: &[u8]) -> String {
        let mut printer = Printer::new();
        let mut all = printer.push(&[bytes, &[0x1d, b'V', 0x42, 0]].concat());
        all.extend(printer.finish());
        text_of(&all[0], &Options::default())
    }

    #[test]
    fn lines_come_out_as_they_were_printed() {
        assert_eq!(text(b"Hello\nWorld\n"), "Hello\nWorld\n");
    }

    #[test]
    fn a_line_in_two_styles_stays_one_line() {
        assert_eq!(text(b"Total: \x1bE\x01 9.50\n"), "Total:  9.50\n");
    }

    #[test]
    fn alignment_uses_the_columns_of_the_paper() {
        // 48 columns: "ab" centred starts at column 23, and at the right it ends at column 48.
        let centred = text(b"\x1ba\x01ab\n");
        assert_eq!(centred, format!("{}ab\n", " ".repeat(23)));
        let right = text(b"\x1ba\x02ab\n");
        assert_eq!(right, format!("{}ab\n", " ".repeat(46)));
    }

    #[test]
    fn wide_characters_take_the_columns_they_print_over() {
        // Double width: every character takes two columns.
        assert_eq!(text(b"\x1d!\x10ab\x1d!\x00c\n"), "a b c\n");
    }

    #[test]
    fn a_tab_goes_to_the_next_stop() {
        assert_eq!(text(b"ab\tc\n"), format!("ab{}c\n", " ".repeat(6)));
    }

    #[test]
    fn a_position_moves_along_the_line() {
        // ESC $ 120 0: 120 dots from the left, which is 10 columns of 12 dots.
        assert_eq!(text(b"\x1b$\x78\x00x\n"), format!("{}x\n", " ".repeat(10)));
    }

    #[test]
    fn feeds_and_blank_lines_are_blank_lines_and_the_end_is_trimmed() {
        assert_eq!(text(b"a\n\nb\n\x1bd\x02"), "a\n\nb\n");
        assert_eq!(text(b"a\n\x1bd\x02b\n"), "a\n\n\nb\n");
    }

    #[test]
    fn pictures_and_codes_are_tags_where_they_are() {
        let bytes = [
            &b"\x1ba\x01"[..],
            &[0x1d, b'v', b'0', 0, 1, 0, 3, 0, 0xff, 0xff, 0xff],
            b"\x1dkA\x0b12345678901",
        ]
        .concat();
        let out = text(&bytes);
        assert!(out.contains("[image 8x3]"), "{out}");
        assert!(out.contains("[barcode upca: 12345678901"), "{out}");
    }
}
