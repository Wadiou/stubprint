//! Barcodes and two dimensional codes as black and white modules, which a printer draws itself.

use receipt::{Barcode, Pdf417, Qr, Symbol, SymbolKind, Symbology};
use rxing::common::BitMatrix;
use rxing::{BarcodeFormat, EncodeHintValue, EncodeHints, MultiFormatWriter, Writer};

/// The modules of a code, a module being a dot of the code and not of the paper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modules {
    pub width: u32,
    pub height: u32,
    bits: Vec<bool>,
}

impl Modules {
    fn of(matrix: &BitMatrix) -> Self {
        let (width, height) = (matrix.width(), matrix.height());
        let bits = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| matrix.get(x, y))
            .collect();
        Self {
            width,
            height,
            bits,
        }
    }

    pub fn is_dark(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.bits[(y * self.width + x) as usize]
    }
}

/// Encodes with no quiet zone, since the drawing decides how much white there is around.
fn encode(text: &str, format: BarcodeFormat, hints: EncodeHints) -> Result<Modules, String> {
    let hints = hints.with(EncodeHintValue::Margin("0".into()));
    MultiFormatWriter
        .encode_with_hints(text, &format, 1, 1, &hints)
        .map(|matrix| Modules::of(&matrix))
        .map_err(|error| error.to_string())
}

pub fn barcode(barcode: &Barcode) -> Result<Modules, String> {
    let mut hints = EncodeHints::default();
    let format = match barcode.symbology {
        Symbology::UpcA => BarcodeFormat::UPC_A,
        Symbology::UpcE => BarcodeFormat::UPC_E,
        Symbology::Ean13 => BarcodeFormat::EAN_13,
        Symbology::Ean8 => BarcodeFormat::EAN_8,
        Symbology::Code39 => BarcodeFormat::CODE_39,
        Symbology::Itf => BarcodeFormat::ITF,
        Symbology::Codabar => BarcodeFormat::CODABAR,
        Symbology::Code93 => BarcodeFormat::CODE_93,
        Symbology::Code128 => BarcodeFormat::CODE_128,
        Symbology::Gs1_128 => {
            hints = hints.with(EncodeHintValue::Gs1Format(true));
            BarcodeFormat::CODE_128
        }
        other => return Err(format!("{} is not drawn", other.name())),
    };
    encode(&barcode.data, format, hints)
}

pub fn qr(code: &Qr) -> Result<Modules, String> {
    let hints = EncodeHints::default()
        .with(EncodeHintValue::ErrorCorrection(code.level.name().into()))
        .with(EncodeHintValue::CharacterSet("UTF-8".into()));
    encode(&code.data, BarcodeFormat::QR_CODE, hints)
}

pub fn pdf417(code: &Pdf417) -> Result<Modules, String> {
    let mut hints = EncodeHints::default();
    // The level is a level from 0 to 8 when the mode says so, and a percentage of the data
    // otherwise, which is only roughly the same thing.
    let level = if code.level_mode == 48 {
        code.level.min(8)
    } else {
        (code.level / 12).min(8)
    };
    hints = hints.with(EncodeHintValue::ErrorCorrection(level.to_string()));
    encode(&code.data, BarcodeFormat::PDF_417, hints)
}

pub fn symbol(symbol: &Symbol) -> Result<Modules, String> {
    let text = String::from_utf8_lossy(&symbol.data).into_owned();
    match symbol.kind {
        SymbolKind::Aztec => encode(&text, BarcodeFormat::AZTEC, EncodeHints::default()),
        SymbolKind::DataMatrix => encode(&text, BarcodeFormat::DATA_MATRIX, EncodeHints::default()),
        other => Err(format!("{} is not drawn", other.name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use receipt::{QrLevel, TextPosition};

    fn ean13(data: &str) -> Barcode {
        Barcode {
            symbology: Symbology::Ean13,
            data: data.into(),
            raw: data.as_bytes().to_vec(),
            height: 80,
            width: 3,
            text: TextPosition::Below,
            font: 0,
        }
    }

    fn row(modules: &Modules) -> String {
        (0..modules.width)
            .map(|x| if modules.is_dark(x, 0) { '#' } else { '.' })
            .collect()
    }

    #[test]
    fn an_ean13_is_95_modules_with_the_guard_bars_where_the_standard_puts_them() {
        // From the EAN-13 standard: start 101, centre 01010, end 101, 95 modules in all.
        let modules = barcode(&ean13("4006381333931")).unwrap();
        let bars = row(&modules);
        assert_eq!(modules.width, 95);
        assert!(bars.starts_with("#.#"), "{bars}");
        assert_eq!(&bars[45..50], ".#.#.");
        assert!(bars.ends_with("#.#"), "{bars}");
    }

    #[test]
    fn a_code_that_cannot_be_drawn_says_why() {
        assert!(barcode(&ean13("12")).is_err());
        let mut databar = ean13("0123");
        databar.symbology = Symbology::Gs1DataBarLimited;
        assert!(barcode(&databar).unwrap_err().contains("not drawn"));
    }

    #[test]
    fn a_qr_code_is_square_with_the_finder_pattern_in_three_corners() {
        let modules = qr(&Qr {
            data: "hello".into(),
            model: 2,
            size: 4,
            level: QrLevel::M,
        })
        .unwrap();
        assert_eq!(modules.width, modules.height);
        // Version 1 is 21 modules, and its finder patterns are 7 by 7 with a dark outline.
        assert_eq!(modules.width, 21);
        for (x, y) in [(0, 0), (6, 0), (0, 6), (14, 0), (20, 6), (0, 14), (6, 20)] {
            assert!(modules.is_dark(x, y), "{x},{y}");
        }
        assert!(!modules.is_dark(1, 1));
        assert!(modules.is_dark(3, 3));
    }
}
