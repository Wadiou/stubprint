use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// A 1-bit image laid out the way a printer reads it: each row is `row_bytes` bytes, the leftmost
/// dot is the highest bit of the first byte, and a set bit is a black dot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Bitmap {
    width: u32,
    height: u32,
    row_bytes: usize,
    /// In base64 when written as JSON.
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    data: Vec<u8>,
}

/// Written as its size and its rows in base64, which is the form a printer reads.
impl Serialize for Bitmap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut bitmap = serializer.serialize_struct("Bitmap", 4)?;
        bitmap.serialize_field("width", &self.width)?;
        bitmap.serialize_field("height", &self.height)?;
        bitmap.serialize_field("row_bytes", &self.row_bytes)?;
        bitmap.serialize_field("data", &STANDARD.encode(&self.data))?;
        bitmap.end()
    }
}

impl Bitmap {
    /// An all white bitmap.
    pub fn new(width: u32, height: u32) -> Self {
        let row_bytes = (width as usize).div_ceil(8);
        Self {
            width,
            height,
            row_bytes,
            data: vec![0; row_bytes * height as usize],
        }
    }

    /// A bitmap from rows already packed the way a printer sends them. The width is the number of
    /// bytes per row times 8. Returns nothing if the data is not exactly `row_bytes * height`.
    pub fn from_packed(row_bytes: usize, height: u32, data: Vec<u8>) -> Option<Self> {
        if row_bytes == 0 || height == 0 || data.len() != row_bytes * height as usize {
            return None;
        }
        Some(Self {
            width: (row_bytes * 8) as u32,
            height,
            row_bytes,
            data,
        })
    }

    /// A bitmap from rows of `width` dots, the way a printer sends a picture in rows. Each row is
    /// padded to whole bytes. Returns nothing if the data is not exactly one row of
    /// `ceil(width / 8)` bytes for each of the `height` rows.
    pub fn from_rows(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        let row_bytes = (width as usize).div_ceil(8);
        if width == 0 || height == 0 || data.len() != row_bytes * height as usize {
            return None;
        }
        Some(Self {
            width,
            height,
            row_bytes,
            data,
        })
    }

    /// A bitmap from columns, the way a printer sends a picture in strips. Each column is
    /// `ceil(height / 8)` bytes, the top dot in the highest bit of the first byte, and the bits
    /// below `height` in the last byte are not used. Returns nothing if the data is not exactly
    /// `width` columns.
    pub fn from_columns(width: u32, height: u32, data: &[u8]) -> Option<Self> {
        let column_bytes = (height as usize).div_ceil(8);
        if width == 0 || height == 0 || data.len() != width as usize * column_bytes {
            return None;
        }
        let mut bitmap = Bitmap::new(width, height);
        for (x, column) in data.chunks(column_bytes).enumerate() {
            for (index, byte) in column.iter().enumerate() {
                for bit in 0..8 {
                    if byte & (0x80 >> bit) != 0 {
                        bitmap.set_black(x as u32, (index * 8 + bit) as u32);
                    }
                }
            }
        }
        Some(bitmap)
    }

    /// The same dots, each repeated `across` times along a row and `down` times down a column. A
    /// printer that prints a picture at half the density does this to the dots.
    pub fn scaled(&self, across: u32, down: u32) -> Bitmap {
        if across <= 1 && down <= 1 {
            return self.clone();
        }
        let (across, down) = (across.max(1), down.max(1));
        let mut scaled = Bitmap::new(self.width * across, self.height * down);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.is_black(x, y) {
                    for dy in 0..down {
                        for dx in 0..across {
                            scaled.set_black(x * across + dx, y * down + dy);
                        }
                    }
                }
            }
        }
        scaled
    }

    /// Makes every dot black that is black in `other`, where the two overlap.
    pub fn merge(&mut self, other: &Bitmap) {
        for y in 0..self.height.min(other.height) {
            for x in 0..self.width.min(other.width) {
                if other.is_black(x, y) {
                    self.set_black(x, y);
                }
            }
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// The packed rows, top to bottom.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// True when the dot at `x`,`y` is black. Dots outside the bitmap are white.
    pub fn is_black(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let byte = self.data[y as usize * self.row_bytes + (x / 8) as usize];
        byte & (0x80 >> (x % 8)) != 0
    }

    /// Makes the dot at `x`,`y` black. Dots outside the bitmap are ignored.
    pub fn set_black(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.data[y as usize * self.row_bytes + (x / 8) as usize] |= 0x80 >> (x % 8);
    }

    /// Puts bitmaps one under the other. Narrower ones are padded with white on the right.
    /// Returns nothing for an empty list.
    pub fn stack(bitmaps: &[Bitmap]) -> Option<Bitmap> {
        let width = bitmaps.iter().map(|bitmap| bitmap.width).max()?;
        let height = bitmaps.iter().map(|bitmap| bitmap.height).sum();
        let mut stacked = Bitmap::new(width, height);
        let mut top = 0usize;
        for bitmap in bitmaps {
            for row in 0..bitmap.height as usize {
                let from = row * bitmap.row_bytes;
                let to = (top + row) * stacked.row_bytes;
                stacked.data[to..to + bitmap.row_bytes]
                    .copy_from_slice(&bitmap.data[from..from + bitmap.row_bytes]);
            }
            top += bitmap.height as usize;
        }
        Some(stacked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftmost_dot_is_the_highest_bit() {
        let mut bitmap = Bitmap::new(8, 1);
        bitmap.set_black(0, 0);
        assert_eq!(bitmap.data(), &[0b1000_0000]);
        bitmap.set_black(7, 0);
        assert_eq!(bitmap.data(), &[0b1000_0001]);
    }

    #[test]
    fn rows_are_padded_to_whole_bytes() {
        let bitmap = Bitmap::new(9, 2);
        assert_eq!(bitmap.row_bytes(), 2);
        assert_eq!(bitmap.data().len(), 4);
    }

    #[test]
    fn dots_outside_are_white_and_cannot_be_set() {
        let mut bitmap = Bitmap::new(4, 4);
        bitmap.set_black(4, 0);
        bitmap.set_black(0, 4);
        assert!(!bitmap.is_black(4, 0));
        assert!(bitmap.data().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn packed_data_must_match_the_size() {
        assert!(Bitmap::from_packed(2, 2, vec![0; 4]).is_some());
        assert!(Bitmap::from_packed(2, 2, vec![0; 3]).is_none());
        assert!(Bitmap::from_packed(0, 2, vec![]).is_none());
        assert!(Bitmap::from_packed(2, 0, vec![]).is_none());
    }

    #[test]
    fn stacking_keeps_rows_in_order_and_pads_narrow_bitmaps() {
        let mut narrow = Bitmap::new(8, 1);
        narrow.set_black(0, 0);
        let mut wide = Bitmap::new(16, 1);
        wide.set_black(15, 0);
        let stacked = Bitmap::stack(&[narrow, wide]).unwrap();
        assert_eq!((stacked.width(), stacked.height()), (16, 2));
        assert!(stacked.is_black(0, 0));
        assert!(!stacked.is_black(8, 0));
        assert!(stacked.is_black(15, 1));
    }

    #[test]
    fn rows_keep_the_exact_width() {
        let bitmap = Bitmap::from_rows(10, 2, vec![0b1000_0000, 0b0100_0000, 0, 0]).unwrap();
        assert_eq!(
            (bitmap.width(), bitmap.height(), bitmap.row_bytes()),
            (10, 2, 2)
        );
        assert!(bitmap.is_black(0, 0));
        assert!(bitmap.is_black(9, 0));
        assert!(Bitmap::from_rows(10, 2, vec![0; 3]).is_none());
    }

    #[test]
    fn columns_run_top_to_bottom_and_left_to_right() {
        // Two columns of one byte: the top dot of the first, the bottom dot of the second.
        let bitmap = Bitmap::from_columns(2, 8, &[0b1000_0000, 0b0000_0001]).unwrap();
        assert_eq!((bitmap.width(), bitmap.height()), (2, 8));
        assert!(bitmap.is_black(0, 0));
        assert!(bitmap.is_black(1, 7));
        assert!(!bitmap.is_black(1, 0));
        assert!(Bitmap::from_columns(2, 8, &[0]).is_none());
        // A height that is not a multiple of 8 leaves the unused bits of the last byte out.
        let short = Bitmap::from_columns(1, 3, &[0b1110_0000]).unwrap();
        assert_eq!(short.height(), 3);
        assert!(short.is_black(0, 2));
    }

    #[test]
    fn scaling_repeats_each_dot() {
        let mut bitmap = Bitmap::new(2, 1);
        bitmap.set_black(1, 0);
        let scaled = bitmap.scaled(2, 3);
        assert_eq!((scaled.width(), scaled.height()), (4, 3));
        for y in 0..3 {
            assert!(!scaled.is_black(0, y) && !scaled.is_black(1, y));
            assert!(scaled.is_black(2, y) && scaled.is_black(3, y));
        }
        assert_eq!(bitmap.scaled(1, 1), bitmap);
    }

    #[test]
    fn merging_adds_black_dots() {
        let mut first = Bitmap::new(8, 1);
        first.set_black(0, 0);
        let mut second = Bitmap::new(8, 1);
        second.set_black(7, 0);
        first.merge(&second);
        assert!(first.is_black(0, 0) && first.is_black(7, 0));
    }

    #[test]
    fn stacking_nothing_gives_nothing() {
        assert_eq!(Bitmap::stack(&[]), None);
    }
}
