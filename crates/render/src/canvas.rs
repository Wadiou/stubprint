//! A grey picture that grows downwards.

/// 8 bit grey, 255 is white paper and 0 is black print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![255; width as usize * height as usize],
        }
    }

    /// More white rows at the bottom.
    pub fn grow_to(&mut self, height: u32) {
        if height > self.height {
            self.pixels
                .resize(self.width as usize * height as usize, 255);
            self.height = height;
        }
    }

    pub fn get(&self, x: i64, y: i64) -> Option<u8> {
        self.index(x, y).map(|index| self.pixels[index])
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && x < i64::from(self.width) && y < i64::from(self.height))
            .then(|| y as usize * self.width as usize + x as usize)
    }

    /// Puts ink on a dot, as much as `strength` from 0 to 1 says. Black ink darkens and white ink
    /// lightens, which is how a white character is drawn on a black cell.
    pub fn ink(&mut self, x: i64, y: i64, strength: f32, black: bool) {
        if let Some(index) = self.index(x, y) {
            let was = f32::from(self.pixels[index]);
            let now = if black {
                was * (1.0 - strength)
            } else {
                was + (255.0 - was) * strength
            };
            self.pixels[index] = now.round() as u8;
        }
    }

    pub fn rect(&mut self, x: i64, y: i64, width: i64, height: i64, black: bool) {
        for row in y..y + height {
            for column in x..x + width {
                self.ink(column, row, 1.0, black);
            }
        }
    }

    /// Black where the other is darker than it is white, used to put a part drawn on its own onto
    /// the paper.
    pub fn lay(&mut self, other: &Canvas, x: i64, y: i64) {
        for row in 0..other.height {
            for column in 0..other.width {
                let value = other.pixels[row as usize * other.width as usize + column as usize];
                if value < 255 {
                    self.ink(
                        x + i64::from(column),
                        y + i64::from(row),
                        1.0 - f32::from(value) / 255.0,
                        true,
                    );
                }
            }
        }
    }

    /// The picture turned half a turn.
    pub fn turned_around(&self) -> Canvas {
        let mut pixels = self.pixels.clone();
        pixels.reverse();
        Canvas {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// Every dot as a block of `factor` by `factor` dots.
    pub fn enlarged(&self, factor: u32) -> Canvas {
        if factor <= 1 {
            return self.clone();
        }
        let mut out = Canvas::new(self.width * factor, self.height * factor);
        for y in 0..out.height {
            for x in 0..out.width {
                out.pixels[(y * out.width + x) as usize] =
                    self.pixels[((y / factor) * self.width + x / factor) as usize];
            }
        }
        out
    }

    pub fn to_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&self.pixels)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_darkens_and_lightens_by_strength() {
        let mut canvas = Canvas::new(2, 1);
        canvas.ink(0, 0, 0.5, true);
        assert_eq!(canvas.get(0, 0), Some(128));
        canvas.ink(0, 0, 1.0, true);
        assert_eq!(canvas.get(0, 0), Some(0));
        canvas.ink(0, 0, 1.0, false);
        assert_eq!(canvas.get(0, 0), Some(255));
        // Off the paper is nothing.
        canvas.ink(-1, 0, 1.0, true);
        canvas.ink(2, 0, 1.0, true);
        assert_eq!(canvas.get(1, 0), Some(255));
    }

    #[test]
    fn a_picture_turned_around_has_its_dots_the_other_way() {
        let mut canvas = Canvas::new(3, 2);
        canvas.ink(0, 0, 1.0, true);
        let turned = canvas.turned_around();
        assert_eq!(turned.get(2, 1), Some(0));
        assert_eq!(turned.get(0, 0), Some(255));
    }

    #[test]
    fn enlarging_repeats_every_dot() {
        let mut canvas = Canvas::new(2, 1);
        canvas.ink(1, 0, 1.0, true);
        let big = canvas.enlarged(2);
        assert_eq!((big.width, big.height), (4, 2));
        assert_eq!(big.get(2, 1), Some(0));
        assert_eq!(big.get(1, 1), Some(255));
    }

    #[test]
    fn it_grows_with_white_at_the_bottom() {
        let mut canvas = Canvas::new(2, 1);
        canvas.ink(0, 0, 1.0, true);
        canvas.grow_to(3);
        assert_eq!((canvas.height, canvas.pixels.len()), (3, 6));
        assert_eq!(canvas.get(0, 0), Some(0));
        assert_eq!(canvas.get(0, 2), Some(255));
    }

    #[test]
    fn a_png_has_the_signature_and_the_size() {
        let png = Canvas::new(5, 7).to_png().unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The IHDR chunk follows: width then height, 4 bytes each.
        assert_eq!(&png[16..24], &[0, 0, 0, 5, 0, 0, 0, 7]);
    }
}
