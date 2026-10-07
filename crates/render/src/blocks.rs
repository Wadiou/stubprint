//! Turns the events of a receipt into what is drawn, top to bottom: lines of text, feeds,
//! pictures and codes, with every size worked out in printer dots. The text drawing and the
//! picture drawing both read these, so they agree on what is on the paper.

use std::collections::BTreeMap;

use receipt::{
    Align, Barcode, Event, FeedAmount, LineSpacing, Pdf417, Position, Qr, Receipt, Style, Symbol,
    UserCharacter, UserKanji,
};

use crate::Options;

/// Printer dots in an inch. Epson and Star printers have 180, 203 or 360, and 203 is the usual.
pub const DPI: u32 = 203;

/// What a line holds, in the order it was printed.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text {
        text: String,
        style: Style,
    },
    /// The tab character: to the next tab stop.
    Tab,
    /// A move of the print position along the line.
    Move {
        absolute: bool,
        dots: i32,
    },
}

/// One line of the paper.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub parts: Vec<Part>,
    pub align: Align,
    /// Where the print area starts, from the left edge of the paper, in dots.
    pub left: u32,
    /// How wide the print area is, in dots.
    pub area: u32,
    /// How far apart the lines are, in dots.
    pub pitch: u32,
    /// A line feed ended it, which moves the paper by the line pitch. Without one the paper moves
    /// only as far as the line is tall.
    pub ended: bool,
    /// The columns the tab character jumps to.
    pub tab_stops: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Line(Line),
    /// The paper moved this many dots, with nothing printed.
    Feed(u32),
    Image(receipt::Bitmap, Align),
    Barcode(Barcode, Align),
    Qr(Qr, Align),
    Pdf417(Pdf417, Align),
    Symbol(Symbol, Align),
    Cut,
}

/// A character a program drew: columns of dots.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub width: usize,
    pub height: usize,
    /// Each column, top dot in the highest bit of the first byte.
    pub columns: Vec<Vec<u8>>,
}

impl Glyph {
    pub fn is_black(&self, x: usize, y: usize) -> bool {
        self.columns
            .get(x)
            .and_then(|column| column.get(y / 8))
            .is_some_and(|byte| byte & (0x80 >> (y % 8)) != 0)
    }

    fn from_columns(width: usize, height: usize, data: &[u8]) -> Self {
        let size = height / 8;
        Self {
            width,
            height,
            columns: data
                .chunks(size.max(1))
                .take(width)
                .map(<[u8]>::to_vec)
                .collect(),
        }
    }
}

/// Everything a receipt draws.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
    /// The characters the program drew, by the private use character that stands for each.
    pub glyphs: BTreeMap<char, Glyph>,
}

struct Builder<'a> {
    options: &'a Options,
    document: Document,
    parts: Vec<Part>,
    /// The alignment of the first text of the line being built.
    line_align: Option<Align>,
    align: Align,
    left_units: u16,
    width_units: Option<u16>,
    x_per_inch: u32,
    y_per_inch: u32,
    spacing: u32,
    tab_stops: Vec<u8>,
}

impl<'a> Builder<'a> {
    fn new(options: &'a Options) -> Self {
        let mut builder = Self {
            options,
            document: Document::default(),
            parts: Vec::new(),
            line_align: None,
            align: Align::Left,
            left_units: 0,
            width_units: None,
            x_per_inch: DPI,
            y_per_inch: DPI,
            spacing: 0,
            tab_stops: Vec::new(),
        };
        builder.reset();
        builder
    }

    /// The settings a printer has when it is switched on or reset.
    fn reset(&mut self) {
        self.align = Align::Left;
        self.left_units = 0;
        self.width_units = None;
        self.x_per_inch = DPI;
        self.y_per_inch = DPI;
        self.spacing = DPI / 6;
        // Every 8 characters, as a printer has them when none are set.
        self.tab_stops = (1..=31).map(|n| n * 8).collect();
    }

    fn horizontal(&self, units: i64) -> i32 {
        (units * i64::from(DPI) / i64::from(self.x_per_inch)) as i32
    }

    fn vertical(&self, units: i64) -> u32 {
        (units * i64::from(DPI) / i64::from(self.y_per_inch)).max(0) as u32
    }

    fn left(&self) -> u32 {
        (self.horizontal(i64::from(self.left_units)).max(0) as u32).min(self.options.width)
    }

    fn area(&self) -> u32 {
        let left = self.left();
        let room = self.options.width - left;
        match self.width_units {
            Some(units) => (self.horizontal(i64::from(units)).max(1) as u32).min(room),
            None => room,
        }
    }

    fn add_text(&mut self, text: &str, style: &Style, align: Align) {
        self.line_align.get_or_insert(align);
        let mut pieces = text.split('\t').peekable();
        while let Some(piece) = pieces.next() {
            if !piece.is_empty() {
                self.parts.push(Part::Text {
                    text: piece.to_string(),
                    style: style.clone(),
                });
            }
            if pieces.peek().is_some() {
                self.parts.push(Part::Tab);
            }
        }
    }

    /// Puts the line being built on the paper.
    fn end_line(&mut self, ended: bool) {
        if self.parts.is_empty() && !ended {
            return;
        }
        let line = Line {
            parts: std::mem::take(&mut self.parts),
            align: self.line_align.take().unwrap_or(self.align),
            left: self.left(),
            area: self.area(),
            pitch: self.spacing,
            ended,
            tab_stops: self.tab_stops.clone(),
        };
        self.document.blocks.push(Block::Line(line));
    }

    fn push(&mut self, block: Block) {
        self.end_line(false);
        self.document.blocks.push(block);
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::Initialize => {
                self.end_line(false);
                self.reset();
            }
            Event::Align(align) => self.align = *align,
            Event::Text { text, style, align } => {
                if !text.is_empty() {
                    self.add_text(text, style, *align);
                }
            }
            Event::LineFeed => self.end_line(true),
            Event::Position(position) => {
                let (absolute, units) = match position {
                    Position::Absolute(units) => (true, i64::from(*units)),
                    Position::Relative(units) => (false, i64::from(*units)),
                };
                let dots = self.horizontal(units);
                self.parts.push(Part::Move { absolute, dots });
            }
            Event::Margin(units) => {
                self.end_line(false);
                self.left_units = *units;
            }
            Event::PrintWidth(units) => {
                self.end_line(false);
                self.width_units = Some(*units);
            }
            Event::MotionUnits { x, y } => {
                self.x_per_inch = if *x == 0 { DPI } else { u32::from(*x) };
                self.y_per_inch = if *y == 0 { DPI } else { u32::from(*y) };
            }
            Event::TabStops(stops) => self.tab_stops = stops.clone(),
            Event::LineSpacing(spacing) => {
                self.spacing = match spacing {
                    LineSpacing::Default => DPI / 6,
                    LineSpacing::Units(n) => self.vertical(i64::from(*n)),
                    LineSpacing::Sixtieths(n) => u32::from(*n) * DPI / 60,
                    LineSpacing::ThreeSixtieths(n) => u32::from(*n) * DPI / 360,
                    LineSpacing::Dots(n) => u32::from(*n),
                };
            }
            Event::Feed(feed) => {
                self.end_line(false);
                let dots = match feed.amount {
                    _ if feed.reverse => 0,
                    FeedAmount::Lines(n) => u32::from(n) * self.spacing,
                    FeedAmount::Units(n) => self.vertical(i64::from(n)),
                    FeedAmount::Page => 0,
                };
                self.document.blocks.push(Block::Feed(dots));
            }
            Event::Image(bitmap) => self.push(Block::Image(bitmap.clone(), self.align)),
            Event::Barcode(barcode) => self.push(Block::Barcode(barcode.clone(), self.align)),
            Event::Qr(qr) => self.push(Block::Qr(qr.clone(), self.align)),
            Event::Pdf417(code) => self.push(Block::Pdf417(code.clone(), self.align)),
            Event::Symbol(symbol) => self.push(Block::Symbol(symbol.clone(), self.align)),
            Event::Cut(_) => self.push(Block::Cut),
            Event::DefineCharacter(character) => self.define_character(character),
            Event::DefineKanji(kanji) => self.define_kanji(kanji),
            // Settings that change nothing on a picture of the paper, and page mode, which is not
            // drawn.
            _ => {}
        }
    }

    fn define_character(&mut self, character: &UserCharacter) {
        let glyph = Glyph::from_columns(
            usize::from(character.width),
            usize::from(character.column_bytes) * 8,
            &character.data,
        );
        self.document
            .glyphs
            .insert(UserCharacter::glyph(character.code), glyph);
    }

    fn define_kanji(&mut self, kanji: &UserKanji) {
        if let Some(character) = UserKanji::glyph(kanji.code) {
            let glyph = Glyph::from_columns(
                usize::from(kanji.width),
                usize::from(kanji.height),
                &kanji.data,
            );
            self.document.glyphs.insert(character, glyph);
        }
    }
}

/// What a receipt draws, worked out for a paper.
pub fn document(receipt: &Receipt, options: &Options) -> Document {
    let mut builder = Builder::new(options);
    for event in receipt.events() {
        builder.event(event);
    }
    builder.end_line(false);
    builder.document
}

#[cfg(test)]
mod tests {
    use super::*;
    use receipt::Printer;

    fn receipt(bytes: &[u8]) -> Receipt {
        let mut printer = Printer::new();
        let mut all = printer.push(&[bytes, &[0x1d, b'V', 0x42, 0]].concat());
        all.extend(printer.finish());
        all.remove(0)
    }

    fn lines(document: &Document) -> Vec<&Line> {
        document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Line(line) => Some(line),
                _ => None,
            })
            .collect()
    }

    fn text_of(line: &Line) -> String {
        line.parts
            .iter()
            .map(|part| match part {
                Part::Text { text, .. } => text.as_str(),
                Part::Tab => "\t",
                Part::Move { .. } => "",
            })
            .collect()
    }

    #[test]
    fn parts_of_a_line_in_different_styles_are_one_line() {
        let document = document(&receipt(b"a\x1bE\x01b\n"), &Options::default());
        let lines = lines(&document);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].parts.len(), 2);
        assert!(lines[0].ended);
    }

    #[test]
    fn a_blank_line_is_a_line_with_nothing_on_it() {
        let document = document(&receipt(b"a\n\nb\n"), &Options::default());
        let lines = lines(&document);
        assert_eq!(lines.len(), 3);
        assert!(lines[1].parts.is_empty());
    }

    #[test]
    fn a_tab_is_a_part_of_its_own() {
        let document = document(&receipt(b"a\tb\n"), &Options::default());
        assert_eq!(text_of(lines(&document)[0]), "a\tb");
        assert!(lines(&document)[0].parts.contains(&Part::Tab));
    }

    #[test]
    fn the_print_area_follows_the_margin_and_the_width() {
        // GS L 100 0 sets the left margin and GS W 200 0 the width, in dots by default.
        let document = document(
            &receipt(b"\x1dL\x64\x00\x1dW\xc8\x00x\n"),
            &Options::default(),
        );
        let line = lines(&document)[0].clone();
        assert_eq!((line.left, line.area), (100, 200));
    }

    #[test]
    fn the_paper_moves_by_what_a_feed_says() {
        // ESC d 3 feeds 3 lines, ESC J 10 feeds 10 vertical units, which are dots by default.
        let document = document(&receipt(b"x\n\x1bd\x03\x1bJ\x0a"), &Options::default());
        let feeds: Vec<u32> = document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Feed(dots) => Some(*dots),
                _ => None,
            })
            .collect();
        assert_eq!(feeds, [3 * (DPI / 6), 10]);
    }

    #[test]
    fn a_character_a_program_drew_is_kept_to_be_drawn() {
        // ESC & 3 65 65: one character, 2 columns of 3 bytes, then the dots.
        let bytes = [
            &b"\x1b&\x03\x41\x41\x02"[..],
            &[0x80, 0, 0, 0, 0, 1],
            b"\x1b%\x01A\n",
        ]
        .concat();
        let document = document(&receipt(&bytes), &Options::default());
        let glyph = &document.glyphs[&UserCharacter::glyph(0x41)];
        assert_eq!((glyph.width, glyph.height), (2, 24));
        assert!(glyph.is_black(0, 0));
        assert!(glyph.is_black(1, 23));
        assert!(!glyph.is_black(1, 0));
    }
}
