//! Draws a receipt as plain text or as a picture of the paper.
//!
//! Both start from [`blocks`], which works out what is on the paper, top to bottom, from the events
//! of a [`receipt::Receipt`].

pub mod blocks;
mod text;

use receipt::{Profile, Receipt};

pub use text::text;

/// What the paper is like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// How wide the paper prints, in dots.
    pub width: u32,
    /// How many characters fit on a line in each font, font A first.
    pub columns: Vec<u8>,
}

impl Default for Options {
    /// Paper 80 mm wide, which prints 576 dots, with the usual 48 and 64 columns.
    fn default() -> Self {
        Self {
            width: 576,
            columns: vec![48, 64],
        }
    }
}

impl Options {
    /// The paper of a printer model: the width it prints, and the columns of its fonts.
    pub fn for_profile(profile: &Profile) -> Self {
        let default = Self::default();
        Self {
            width: profile
                .width_dots
                .filter(|width| *width > 0)
                .unwrap_or(default.width),
            columns: if profile.columns.is_empty() {
                default.columns
            } else {
                profile.columns.clone()
            },
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
