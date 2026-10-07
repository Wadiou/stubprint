//! The fonts, which are inside the program, and the drawing of one character.

use std::sync::OnceLock;

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

use crate::canvas::Canvas;

struct Set {
    latin: FontRef<'static>,
    latin_bold: FontRef<'static>,
    arabic: FontRef<'static>,
    arabic_bold: FontRef<'static>,
    thai: FontRef<'static>,
    cjk: FontRef<'static>,
}

fn load(bytes: &'static [u8]) -> FontRef<'static> {
    FontRef::try_from_slice(bytes).expect("the fonts inside the program are valid")
}

fn set() -> &'static Set {
    static SET: OnceLock<Set> = OnceLock::new();
    SET.get_or_init(|| Set {
        latin: load(include_bytes!("../fonts/NotoSansMono_400Regular.ttf")),
        latin_bold: load(include_bytes!("../fonts/NotoSansMono_700Bold.ttf")),
        arabic: load(include_bytes!("../fonts/NotoNaskhArabic_400Regular.ttf")),
        arabic_bold: load(include_bytes!("../fonts/NotoNaskhArabic_700Bold.ttf")),
        thai: load(include_bytes!("../fonts/NotoSansThai_400Regular.ttf")),
        cjk: load(include_bytes!("../fonts/NotoSansJP_400Regular_subset.ttf")),
    })
}

/// The first font that has a glyph for the character, and how many cells it takes: the wide
/// characters of East Asia take two.
fn find(character: char, bold: bool) -> (&'static FontRef<'static>, usize) {
    let fonts = set();
    let (latin, arabic) = if bold {
        (&fonts.latin_bold, &fonts.arabic_bold)
    } else {
        (&fonts.latin, &fonts.arabic)
    };
    let has = |font: &FontRef<'static>| font.glyph_id(character).0 != 0;
    if has(latin) {
        (latin, 1)
    } else if has(arabic) {
        (arabic, 1)
    } else if has(&fonts.thai) {
        (&fonts.thai, 1)
    } else if has(&fonts.cjk) {
        (&fonts.cjk, columns(character))
    } else {
        // Nothing has it: the box of the font that prints missing characters, as a printer prints
        // a mark for a character it does not know.
        (latin, 1)
    }
}

/// How many cells a character takes.
pub fn columns(character: char) -> usize {
    match u32::from(character) {
        0x1100..=0x115f
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6 => 2,
        0x0300..=0x036f | 0x064b..=0x065f | 0x200b..=0x200f => 0,
        _ => 1,
    }
}

/// What a printed character looks like: where it goes and how big its cell is.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    /// The left edge of the character's cell, in dots.
    pub x: i64,
    /// The baseline, in dots.
    pub baseline: i64,
    /// How wide one cell is, with the size multiplier applied.
    pub width: u32,
    /// How tall one cell is, with the size multiplier applied.
    pub height: u32,
}

/// Draws a character into its cell. The font is sized so that a Latin character is as wide as the
/// cell, which is how a printer's own fixed width font looks.
pub fn draw(canvas: &mut Canvas, character: char, cell: Cell, bold: bool, black: bool) {
    let (font, cells) = find(character, bold);
    let latin = &set().latin;
    // A Latin character is 0.6 of its size wide, so that size fits the cell. A wide character is
    // as wide as it is high, and takes two cells.
    let advance =
        latin.h_advance_unscaled(latin.glyph_id('M')) / latin.units_per_em().unwrap_or(1000.0);
    let size = cell.width as f32 / advance;
    let scale = PxScale {
        x: if cells == 2 {
            size * advance * 2.0
        } else {
            size
        },
        // A cell is twice as tall as it is wide, and a taller or flatter one stretches the font.
        y: size * (cell.height as f32 / (cell.width as f32 * 2.0)),
    };
    let scaled = font.as_scaled(scale);
    let glyph = font.glyph_id(character);
    // The character is centred in its cell, which matters for the narrow and the wide ones.
    let drawn = scaled.h_advance(glyph);
    let room = (cell.width as usize * cells.max(1)) as f32;
    let left = cell.x as f32 + ((room - drawn) / 2.0).max(0.0);
    let glyph = glyph.with_scale_and_position(scale, point(left, cell.baseline as f32));
    if let Some(outline) = font.outline_glyph(glyph) {
        let bounds = outline.px_bounds();
        outline.draw(|x, y, coverage| {
            canvas.ink(
                i64::from(bounds.min.x as i32) + i64::from(x),
                i64::from(bounds.min.y as i32) + i64::from(y),
                coverage,
                black,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dots(character: char) -> usize {
        let mut canvas = Canvas::new(48, 48);
        draw(
            &mut canvas,
            character,
            Cell {
                x: 12,
                baseline: 30,
                width: 12,
                height: 24,
            },
            false,
            true,
        );
        canvas.pixels.iter().filter(|p| **p < 128).count()
    }

    #[test]
    fn letters_have_ink_and_a_space_has_none() {
        assert!(dots('A') > 20);
        assert_eq!(dots(' '), 0);
    }

    #[test]
    fn the_fonts_cover_latin_arabic_thai_and_japanese() {
        for character in ['é', 'ع', 'ก', '中', 'ア', '€', '─'] {
            assert!(dots(character) > 5, "{character}");
        }
    }

    #[test]
    fn a_character_stays_in_its_cell() {
        let mut canvas = Canvas::new(48, 48);
        draw(
            &mut canvas,
            'W',
            Cell {
                x: 12,
                baseline: 30,
                width: 12,
                height: 24,
            },
            true,
            true,
        );
        for y in 0..48 {
            for x in (0..11).chain(25..48) {
                assert_eq!(canvas.get(x, y), Some(255), "{x},{y} is outside the cell");
            }
        }
    }

    #[test]
    fn wide_characters_take_two_cells() {
        assert_eq!(columns('中'), 2);
        assert_eq!(columns('a'), 1);
        assert_eq!(columns('\u{0301}'), 0);
    }
}
