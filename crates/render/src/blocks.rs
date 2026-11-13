//! Turns the events of a receipt into what is drawn, top to bottom: lines of text, feeds,
//! pictures and codes, with every size worked out in printer dots. The text drawing and the
//! picture drawing both read these, so they agree on what is on the paper.

use std::collections::BTreeMap;

use stubprint_core::{
    Align, Barcode, Event, FeedAmount, Layout, LineSpacing, Pdf417, Position, Qr, Receipt, Style,
    Symbol, UserCharacter, UserKanji,
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
    Image(stubprint_core::Bitmap, Align),
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
    /// The paper settings the events have set, which the core follows.
    layout: Layout,
}

impl<'a> Builder<'a> {
    fn new(options: &'a Options) -> Self {
        Self {
            options,
            document: Document::default(),
            parts: Vec::new(),
            line_align: None,
            align: Align::Left,
            layout: Layout::default(),
        }
    }

    fn x_per_inch(&self) -> u32 {
        match self.layout.motion_units.x {
            0 => DPI,
            units => u32::from(units),
        }
    }

    fn y_per_inch(&self) -> u32 {
        match self.layout.motion_units.y {
            0 => DPI,
            units => u32::from(units),
        }
    }

    fn horizontal(&self, units: i64) -> i32 {
        (units * i64::from(DPI) / i64::from(self.x_per_inch())) as i32
    }

    fn vertical(&self, units: i64) -> u32 {
        (units * i64::from(DPI) / i64::from(self.y_per_inch())).max(0) as u32
    }

    fn left(&self) -> u32 {
        (self.horizontal(i64::from(self.layout.margin)).max(0) as u32).min(self.options.width)
    }

    fn area(&self) -> u32 {
        let room = self.options.width - self.left();
        match self.layout.print_width {
            Some(units) => (self.horizontal(i64::from(units)).max(1) as u32).min(room),
            None => room,
        }
    }

    /// How far apart the lines are, in dots.
    fn spacing(&self) -> u32 {
        match self.layout.line_spacing {
            LineSpacing::Default => DPI / 6,
            LineSpacing::Units(n) => self.vertical(i64::from(n)),
            LineSpacing::Sixtieths(n) => u32::from(n) * DPI / 60,
            LineSpacing::ThreeSixtieths(n) => u32::from(n) * DPI / 360,
            LineSpacing::Dots(n) => u32::from(n),
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

    /// Puts the line being built on the paper. A line longer than the print area goes on after a
    /// line feed: the Epson glossary calls it print buffer-full printing.
    fn end_line(&mut self, ended: bool) {
        if self.parts.is_empty() && !ended {
            return;
        }
        let align = self.line_align.take().unwrap_or(self.align);
        let parts = std::mem::take(&mut self.parts);
        let lines = self.wrap(parts);
        let last = lines.len() - 1;
        for (index, parts) in lines.into_iter().enumerate() {
            self.document.blocks.push(Block::Line(Line {
                parts,
                align,
                left: self.left(),
                area: self.area(),
                pitch: self.spacing(),
                ended: ended || index < last,
                tab_stops: self.layout.tab_stops.clone(),
            }));
        }
    }

    /// Cuts the parts of a line into the lines they are printed on. A line placed with moves is
    /// left as it is.
    fn wrap(&self, parts: Vec<Part>) -> Vec<Vec<Part>> {
        let room = i64::from(self.area());
        if parts.iter().any(|part| matches!(part, Part::Move { .. })) {
            return vec![parts];
        }
        let mut lines = Vec::new();
        let mut current = Vec::new();
        let mut cursor = 0i64;
        for part in parts {
            match part {
                Part::Text { text, style } => {
                    let cell =
                        i64::from(self.options.cell_width(style.font) * u32::from(style.width));
                    let mut run = String::new();
                    for character in text.chars() {
                        let cells = crate::fonts::columns(character) as i64;
                        if cells > 0 && cursor > 0 && cursor + cell * cells > room {
                            if !run.is_empty() {
                                current.push(Part::Text {
                                    text: std::mem::take(&mut run),
                                    style: style.clone(),
                                });
                            }
                            lines.push(std::mem::take(&mut current));
                            cursor = 0;
                        }
                        run.push(character);
                        cursor += cell * cells
                            + if cells > 0 {
                                i64::from(style.spacing)
                            } else {
                                0
                            };
                    }
                    if !run.is_empty() {
                        current.push(Part::Text { text: run, style });
                    }
                }
                Part::Tab => {
                    let cell = i64::from(self.options.cell_width(0));
                    if let Some(stop) = self
                        .layout
                        .tab_stops
                        .iter()
                        .map(|stop| i64::from(*stop) * cell)
                        .find(|stop| *stop > cursor)
                    {
                        cursor = stop;
                    }
                    current.push(Part::Tab);
                }
                other => current.push(other),
            }
        }
        lines.push(current);
        lines
    }

    fn push(&mut self, block: Block) {
        self.end_line(false);
        self.document.blocks.push(block);
    }

    fn event(&mut self, event: &Event) {
        // Whatever changes where the next line lands ends the line that was being built.
        if matches!(
            event,
            Event::Initialize | Event::Margin(_) | Event::PrintWidth(_)
        ) {
            self.end_line(false);
        }
        self.layout.apply(event);
        match event {
            Event::Initialize => self.align = Align::Left,
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
            Event::Feed(feed) => {
                self.end_line(false);
                let dots = match feed.amount {
                    _ if feed.reverse => 0,
                    FeedAmount::Lines(n) => u32::from(n) * self.spacing(),
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
/// How much paper had come out, in dots, after each of the events, as a program that prints them
/// one by one would see it. A line comes out when it ends, so the events of a line that is still
/// being written add nothing until it does. It is the same as `paper_height` without the edges, and
/// the number for an event does not depend on the events after it.
pub fn marks<'a>(events: impl IntoIterator<Item = &'a Event>, options: &Options) -> Vec<u32> {
    let mut builder = Builder::new(options);
    let mut marks = Vec::new();
    let mut dots = 0u32;
    let mut counted = 0;
    for event in events {
        builder.event(event);
        for block in &builder.document.blocks[counted..] {
            dots = dots.saturating_add(crate::paint::block_height(block, options));
        }
        counted = builder.document.blocks.len();
        marks.push(dots);
    }
    marks
}

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
    use stubprint_core::Printer;

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

    fn lines_ended(document: &Document) -> Vec<bool> {
        lines(document).iter().map(|line| line.ended).collect()
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
    fn the_paper_that_came_out_is_counted_after_each_event_and_never_depends_on_what_follows() {
        let whole = receipt(b"one\ntwo\n\x1bd\x03three\n\x1dV\x42\x00");
        let options = Options::default();
        let marks = marks(whole.events(), &options);
        assert_eq!(marks.len(), whole.events().len());
        // The paper only goes forward, and a line that is not ended has not come out.
        assert!(marks.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(
            marks[0], 0,
            "the text of a line takes no paper until it ends"
        );
        // The last number is how long the paper is, which the height says with its edges.
        let height = crate::paper_height(&whole, &options);
        assert!(
            height - marks.last().unwrap() <= 16,
            "{height} and {marks:?}"
        );
        // Looking at the first events alone gives the same numbers as looking at all of them.
        let few = marks_of(&whole.events()[..3], &options);
        assert_eq!(few, marks[..3]);
    }

    fn marks_of(events: &[Event], options: &Options) -> Vec<u32> {
        marks(events, options)
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

    fn printed(bytes: &[u8]) -> Vec<String> {
        let document = document(&receipt(bytes), &Options::default());
        lines(&document).into_iter().map(text_of).collect()
    }

    #[test]
    fn a_line_that_fills_the_paper_exactly_stays_one_line() {
        assert_eq!(
            printed(&[b"x".repeat(48), b"\n".to_vec()].concat()),
            ["x".repeat(48)]
        );
    }

    #[test]
    fn a_line_longer_than_the_paper_goes_on_after_a_line_feed() {
        let lines = printed(&[b"x".repeat(49), b"\n".to_vec()].concat());
        assert_eq!(lines, ["x".repeat(48), "x".to_string()]);
        let document = document(
            &receipt(&[b"x".repeat(49), b"\n".to_vec()].concat()),
            &Options::default(),
        );
        assert!(lines_ended(&document).iter().all(|ended| *ended));
    }

    #[test]
    fn double_width_letters_use_two_cells_so_a_line_wraps_at_half_the_columns() {
        let lines = printed(
            &[
                b"\x1d!\x10".to_vec(),
                b"0123456789".repeat(3),
                b"\n".to_vec(),
            ]
            .concat(),
        );
        assert_eq!(lines, ["012345678901234567890123", "456789"]);
    }

    #[test]
    fn a_character_two_cells_wide_that_does_not_fit_starts_the_next_line() {
        // FS & turns Kanji mode on, and JIS 0x3441 is 漢, which takes two cells.
        let bytes = [b"x".repeat(47), b"\x1c&\x34\x41\x1c.\n".to_vec()].concat();
        let lines = printed(&bytes);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "x".repeat(47));
        assert_eq!(lines[1].chars().count(), 1);
    }

    #[test]
    fn a_wrapped_line_keeps_its_styles_and_each_part_of_it_is_aligned_alone() {
        let document = document(
            &receipt(
                &[
                    b"\x1ba\x01".to_vec(),
                    b"a".repeat(30),
                    b"\x1bE\x01".to_vec(),
                    b"b".repeat(30),
                    b"\n".to_vec(),
                ]
                .concat(),
            ),
            &Options::default(),
        );
        let lines = lines(&document);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.align == Align::Center));
        assert_eq!(
            text_of(lines[0]),
            format!("{}{}", "a".repeat(30), "b".repeat(18))
        );
        assert_eq!(text_of(lines[1]), "b".repeat(12));
        assert!(matches!(&lines[1].parts[0], Part::Text { style, .. } if style.bold));
    }

    #[test]
    fn a_line_placed_by_moves_is_not_cut() {
        let lines = printed(&[b"\x1b$\x64\x00".to_vec(), b"x".repeat(60), b"\n".to_vec()].concat());
        assert_eq!(lines, ["x".repeat(60)]);
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
