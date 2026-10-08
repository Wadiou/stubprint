//! Draws a receipt as plain text or as a picture of the paper.
//!
//! Both start from [`blocks`], which works out what is on the paper, top to bottom, from the events
//! of a [`stubprint_core::Receipt`].

pub mod blocks;
mod canvas;
mod codes;
mod fonts;
mod paint;
mod text;

use stubprint_core::{Profile, Receipt};

pub use canvas::Canvas;
pub use text::text;

/// How wide a character of font A is, in dots, on every Epson and Star printer.
const FONT_A_DOTS: u32 = 12;

/// What the paper is like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// How wide the paper prints, in dots.
    pub width: u32,
    /// How many characters fit on a line in each font, font A first.
    pub columns: Vec<u8>,
    /// How many times larger the picture is drawn, from 1 to 4.
    pub scale: u32,
}

impl Default for Options {
    /// Paper 80 mm wide, which prints 576 dots, with the usual 48 and 64 columns.
    fn default() -> Self {
        Self {
            width: 576,
            columns: vec![48, 64],
            scale: 1,
        }
    }
}

impl Options {
    /// The paper of a printer model: the width it prints, and the columns of its fonts.
    pub fn for_profile(profile: &Profile) -> Self {
        let default = Self::default();
        let columns = if profile.columns.is_empty() {
            default.columns
        } else {
            profile.columns.clone()
        };
        // A model that does not say how wide it prints has the columns of font A, which is 12 dots
        // wide: 42 columns are 504 dots, which is what an 80 mm printer prints with the margins.
        let from_columns = u32::from(columns[0]) * FONT_A_DOTS;
        Self {
            width: profile
                .width_dots
                .filter(|width| *width > 0)
                .unwrap_or(from_columns),
            columns,
            scale: 1,
        }
    }

    /// How wide a character of a font is, in dots.
    pub fn cell_width(&self, font: u8) -> u32 {
        let columns = self
            .columns
            .get(usize::from(font))
            .or(self.columns.first())
            .copied()
            .unwrap_or(48);
        (self.width / u32::from(columns.max(1))).max(1)
    }

    /// How tall a character of a font is, in dots.
    pub fn cell_height(&self, font: u8) -> u32 {
        self.cell_width(font) * 2
    }
}

/// The paper as text, for reading and for a program to look at.
pub fn text_of(receipt: &Receipt, options: &Options) -> String {
    text(&blocks::document(receipt, options), options)
}

/// The paper as a picture, in grey levels, one pixel for each dot the printer prints.
pub fn picture(receipt: &Receipt, options: &Options) -> Canvas {
    paint::paint(&blocks::document(receipt, options), options)
}

/// The paper as a PNG file.
pub fn png(receipt: &Receipt, options: &Options) -> Result<Vec<u8>, String> {
    picture(receipt, options)
        .to_png()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use stubprint_core::Profile;

    use super::Options;

    fn profile(columns: &[u8], width_dots: Option<u32>) -> Profile {
        let mut profile = Profile::generic();
        profile.columns = columns.to_vec();
        profile.width_dots = width_dots;
        profile
    }

    #[test]
    fn a_model_that_says_its_width_and_columns_has_them() {
        let options = Options::for_profile(&profile(&[56, 72], Some(640)));
        assert_eq!(options.width, 640);
        assert_eq!(options.columns, vec![56, 72]);
        assert_eq!(options.scale, 1);
    }

    #[test]
    fn a_model_without_a_width_prints_the_columns_of_font_a_at_twelve_dots() {
        assert_eq!(Options::for_profile(&profile(&[42, 56], None)).width, 504);
        // A width of 0 is not a width.
        assert_eq!(
            Options::for_profile(&profile(&[32, 42], Some(0))).width,
            384
        );
        assert_eq!(Options::for_profile(&profile(&[32], Some(1))).width, 1);
    }

    #[test]
    fn a_model_without_columns_has_the_usual_ones() {
        let options = Options::for_profile(&profile(&[], None));
        assert_eq!(options, Options::default());
        assert_eq!(options.width, 48 * 12);
    }
}
