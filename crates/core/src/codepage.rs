use encoding_rs::{
    Encoding, ISO_8859_15, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254,
    WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use oem_cp::code_table::DECODING_TABLE_CP_MAP;

/// How a printer's code page number, set with `ESC t n`, is read.
enum Charset {
    /// An IBM code page, such as 437 or 850, by its usual number.
    Ibm(u16),
    /// A Windows or ISO code page.
    Encoding(&'static Encoding),
}

/// The numbers are the Epson table that receipt libraries write. A number this crate does not know
/// is read as Windows 1252, the closest thing to a default.
fn charset(printer_page: u8) -> Charset {
    match printer_page {
        0 => Charset::Ibm(437),
        2 => Charset::Ibm(850),
        3 => Charset::Ibm(860),
        4 => Charset::Ibm(863),
        5 => Charset::Ibm(865),
        13 => Charset::Ibm(857),
        14 => Charset::Ibm(737),
        17 => Charset::Ibm(866),
        18 => Charset::Ibm(852),
        19 => Charset::Ibm(858),
        32 => Charset::Ibm(720),
        33 => Charset::Ibm(775),
        34 => Charset::Ibm(855),
        35 => Charset::Ibm(861),
        36 => Charset::Ibm(862),
        37 => Charset::Ibm(864),
        38 => Charset::Ibm(869),
        40 => Charset::Encoding(ISO_8859_15),
        45 => Charset::Encoding(WINDOWS_1250),
        46 => Charset::Encoding(WINDOWS_1251),
        47 => Charset::Encoding(WINDOWS_1253),
        48 => Charset::Encoding(WINDOWS_1254),
        49 => Charset::Encoding(WINDOWS_1255),
        50 => Charset::Encoding(WINDOWS_1256),
        51 => Charset::Encoding(WINDOWS_1257),
        52 => Charset::Encoding(WINDOWS_1258),
        _ => Charset::Encoding(WINDOWS_1252),
    }
}

/// Reads the bytes of a printed line in the printer's code page.
pub fn decode(printer_page: u8, bytes: &[u8]) -> String {
    match charset(printer_page) {
        Charset::Encoding(encoding) => encoding.decode_without_bom_handling(bytes).0.into_owned(),
        Charset::Ibm(number) => match DECODING_TABLE_CP_MAP.get(&number) {
            Some(table) => table.decode_string_lossy(bytes),
            None => WINDOWS_1252
                .decode_without_bom_handling(bytes)
                .0
                .into_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_reads_the_same_in_every_page() {
        for page in [0, 2, 16, 37, 50, 99] {
            assert_eq!(decode(page, b"Total 12.50"), "Total 12.50", "page {page}");
        }
    }

    #[test]
    fn cp437_has_accents_and_box_lines() {
        assert_eq!(decode(0, &[0x82, 0x81, 0xa4, 0x87, 0xa8]), "éüñç¿");
        assert_eq!(decode(0, &[0xc4, 0xcd]), "─═");
    }

    #[test]
    fn windows_1252_has_the_euro_and_dashes() {
        assert_eq!(decode(16, &[0x80, 0x97, 0x8c]), "€—Œ");
    }

    #[test]
    fn windows_1256_has_arabic() {
        assert_eq!(decode(50, &[0xe3, 0xd1, 0xcd, 0xc8, 0xc7]), "مرحبا");
    }

    #[test]
    fn cp864_has_arabic_in_its_own_places() {
        // 0xC1 is a hamza in CP864 and 0xA1 is something else, so the pages are told apart.
        assert_ne!(decode(37, &[0xc1]), decode(50, &[0xc1]));
    }

    #[test]
    fn an_unknown_page_falls_back_to_windows_1252() {
        assert_eq!(decode(200, &[0x80]), "€");
    }
}
