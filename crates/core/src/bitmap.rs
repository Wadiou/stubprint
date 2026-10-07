/// A 1-bit image laid out the way a printer reads it: each row is `row_bytes` bytes, the leftmost
/// dot is the highest bit of the first byte, and a set bit is a black dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    row_bytes: usize,
    data: Vec<u8>,
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
    fn stacking_nothing_gives_nothing() {
        assert_eq!(Bitmap::stack(&[]), None);
    }
}
