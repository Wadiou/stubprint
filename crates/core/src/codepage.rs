use std::collections::BTreeSet;

use encoding_rs::{
    Encoding, BIG5, EUC_JP, EUC_KR, GB18030, IBM866, ISO_8859_15, ISO_8859_2, ISO_8859_3,
    ISO_8859_4, ISO_8859_5, ISO_8859_6, ISO_8859_7, ISO_8859_8, SHIFT_JIS, WINDOWS_1250,
    WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256,
    WINDOWS_1257, WINDOWS_1258,
};
use oem_cp::code_table::DECODING_TABLE_CP_MAP;

use crate::event::{UserCharacter, UserKanji};
use crate::profile::KanjiEncoding;

/// How the bytes of a line are read. Printers differ in what the numbers of `ESC t` mean, so the
/// name of the code page comes from the printer's profile.
pub(crate) struct TextContext<'a> {
    /// The code page the printer's profile gives for the number that was selected, such as `CP437`.
    /// With none, the bytes are read as Windows 1252.
    pub page: Option<&'a str>,
    /// The international character set of `ESC R`, 0 for the American one.
    pub international: u8,
    /// The codes of the characters the program drew, when it asked for them to be used.
    pub user_characters: Option<&'a BTreeSet<u8>>,
    /// Set while two byte characters are on.
    pub kanji: Option<TwoByte<'a>>,
    /// Set after `FS ( C` chose UTF-8.
    pub utf8: bool,
}

/// How two byte characters are read, and which of them the program drew.
pub(crate) struct TwoByte<'a> {
    pub encoding: KanjiEncoding,
    /// The codes, as sent, of the characters drawn with `FS 2`.
    pub drawn: &'a BTreeSet<[u8; 2]>,
}

/// How the upper half of a single byte code page is read.
enum Table {
    /// An IBM code page, such as 437 or 850, by its number.
    Ibm(u16),
    Encoding(&'static Encoding),
    /// The single byte half of Shift JIS: the Japanese katakana at 0xA1 to 0xDF.
    Katakana,
}

/// The table for a code page name from a printer profile. A few names have no table here and are
/// read as the closest page there is: CP851, CP853, CP1098, CP1125, CP772, CP774, CP856, RK1048
/// and the Vietnamese pages. Names that are specific to one maker, such as Star's CP3012, are read
/// as Windows 1252.
fn table(name: &str) -> Table {
    let name = name.to_ascii_uppercase();
    if let Some(number) = name.strip_prefix("CP").and_then(|n| n.parse::<u16>().ok()) {
        match number {
            437 | 720 | 737 | 775 | 850 | 852 | 855 | 857 | 858 | 860 | 861 | 862 | 863 | 864
            | 865 | 866 | 869 | 874 => return Table::Ibm(number),
            932 => return Table::Katakana,
            1250 => return Table::Encoding(WINDOWS_1250),
            1251 => return Table::Encoding(WINDOWS_1251),
            1252 => return Table::Encoding(WINDOWS_1252),
            1253 => return Table::Encoding(WINDOWS_1253),
            1254 => return Table::Encoding(WINDOWS_1254),
            1255 => return Table::Encoding(WINDOWS_1255),
            1256 => return Table::Encoding(WINDOWS_1256),
            1257 => return Table::Encoding(WINDOWS_1257),
            1258 => return Table::Encoding(WINDOWS_1258),
            928 => return Table::Encoding(ISO_8859_7),
            // The closest page there is.
            851 => return Table::Encoding(WINDOWS_1253),
            853 => return Table::Encoding(WINDOWS_1254),
            856 => return Table::Encoding(WINDOWS_1255),
            1098 => return Table::Encoding(WINDOWS_1256),
            1125 => return Table::Encoding(IBM866),
            772 | 774 => return Table::Encoding(WINDOWS_1257),
            _ => {}
        }
    }
    match name.as_str() {
        "ISO_8859-2" => Table::Encoding(ISO_8859_2),
        "ISO_8859-3" => Table::Encoding(ISO_8859_3),
        "ISO_8859-4" => Table::Encoding(ISO_8859_4),
        "ISO_8859-5" => Table::Encoding(ISO_8859_5),
        "ISO_8859-6" => Table::Encoding(ISO_8859_6),
        "ISO_8859-7" => Table::Encoding(ISO_8859_7),
        "ISO_8859-8" => Table::Encoding(ISO_8859_8),
        "ISO_8859-9" => Table::Encoding(WINDOWS_1254),
        "ISO_8859-15" => Table::Encoding(ISO_8859_15),
        "RK1048" => Table::Encoding(WINDOWS_1251),
        "TCVN-3-1" | "TCVN-3-2" => Table::Encoding(WINDOWS_1258),
        _ => Table::Encoding(WINDOWS_1252),
    }
}

fn decode_run(table: &Table, bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    match table {
        Table::Encoding(encoding) => encoding.decode_without_bom_handling(bytes).0.into_owned(),
        Table::Ibm(number) => match DECODING_TABLE_CP_MAP.get(number) {
            Some(table) => table.decode_string_lossy(bytes),
            None => WINDOWS_1252
                .decode_without_bom_handling(bytes)
                .0
                .into_owned(),
        },
        Table::Katakana => bytes
            .iter()
            .map(|byte| match byte {
                0x00..=0x7f => char::from(*byte),
                0xa1..=0xdf => {
                    char::from_u32(0xff61 + u32::from(byte - 0xa1)).unwrap_or('\u{fffd}')
                }
                _ => '\u{fffd}',
            })
            .collect(),
    }
}

/// The twelve ASCII characters that an international character set replaces.
const REPLACED: [u8; 12] = [
    0x23, 0x24, 0x40, 0x5b, 0x5c, 0x5d, 0x5e, 0x60, 0x7b, 0x7c, 0x7d, 0x7e,
];

/// The characters of each international character set at the places in `REPLACED`.
const INTERNATIONAL: [[char; 12]; 16] = [
    ['#', '$', '@', '[', '\\', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '$', 'à', '°', 'ç', '§', '^', '`', 'é', 'ù', 'è', '¨'],
    ['#', '$', '§', 'Ä', 'Ö', 'Ü', '^', '`', 'ä', 'ö', 'ü', 'ß'],
    ['£', '$', '@', '[', '\\', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '$', '@', 'Æ', 'Ø', 'Å', '^', '`', 'æ', 'ø', 'å', '~'],
    ['#', '¤', 'É', 'Ä', 'Ö', 'Å', 'Ü', 'é', 'ä', 'ö', 'å', 'ü'],
    ['#', '$', '@', '°', '\\', 'é', '^', 'ù', 'à', 'ò', 'è', 'ì'],
    ['₧', '$', '@', '¡', 'Ñ', '¿', '^', '`', '¨', 'ñ', '}', '~'],
    ['#', '$', '@', '[', '¥', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '¤', 'É', 'Æ', 'Ø', 'Å', 'Ü', 'é', 'æ', 'ø', 'å', 'ü'],
    ['#', '$', 'É', 'Æ', 'Ø', 'Å', 'Ü', 'é', 'æ', 'ø', 'å', 'ü'],
    ['#', '$', 'á', '¡', 'Ñ', '¿', 'é', '`', 'í', 'ñ', 'ó', 'ú'],
    ['#', '$', 'á', '¡', 'Ñ', '¿', 'é', 'ü', 'í', 'ñ', 'ó', 'ú'],
    ['#', '$', '@', '[', '₩', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '$', 'Ž', 'Š', 'Đ', 'Ć', 'Č', 'ž', 'š', 'đ', 'ć', 'č'],
    ['#', '$', '@', '[', '¥', ']', '^', '`', '{', '|', '}', '~'],
];

/// What an international character set prints for an ASCII byte, if it prints something else.
fn international(set: u8, byte: u8) -> Option<char> {
    let row = INTERNATIONAL.get(usize::from(set))?;
    let index = REPLACED.iter().position(|replaced| *replaced == byte)?;
    let character = row[index];
    (character != char::from(byte)).then_some(character)
}

/// Reads bytes below 0x80, and the single bytes of a code page, with the characters the program
/// drew and the international character set taken into account.
fn decode_single(context: &TextContext, table: &Table, bytes: &[u8]) -> String {
    let special = |byte: u8| -> Option<char> {
        if byte >= 0x80 {
            return None;
        }
        if context
            .user_characters
            .is_some_and(|set| set.contains(&byte))
        {
            return Some(UserCharacter::glyph(byte));
        }
        international(context.international, byte)
    };

    let mut text = String::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if let Some(character) = special(*byte) {
            text.push_str(&decode_run(table, &bytes[start..index]));
            text.push(character);
            start = index + 1;
        }
    }
    text.push_str(&decode_run(table, &bytes[start..]));
    text
}

/// How many bytes the character that starts with `bytes[0]` takes, in a two byte encoding.
fn kanji_length(encoding: KanjiEncoding, bytes: &[u8]) -> usize {
    let lead = bytes[0];
    let wanted = match encoding {
        KanjiEncoding::Jis => match (lead, bytes.get(1)) {
            (0x21..=0x7e, Some(0x21..=0x7e)) => 2,
            _ => 1,
        },
        KanjiEncoding::ShiftJis => match lead {
            0x81..=0x9f | 0xe0..=0xfc => 2,
            _ => 1,
        },
        KanjiEncoding::Gb18030 => match lead {
            0x81..=0xfe
                if bytes
                    .get(1)
                    .is_some_and(|byte| (0x30..=0x39).contains(byte)) =>
            {
                4
            }
            0x81..=0xfe => 2,
            _ => 1,
        },
        KanjiEncoding::Big5 | KanjiEncoding::EucKr => match lead {
            0x81..=0xfe => 2,
            _ => 1,
        },
    };
    wanted.min(bytes.len())
}

/// Reads characters of a two byte encoding, one or more of them.
fn decode_wide(encoding: KanjiEncoding, bytes: &[u8]) -> String {
    let (encoding, bytes) = match encoding {
        // The JIS code is the EUC-JP code with the top bit of each byte cleared.
        KanjiEncoding::Jis => (
            EUC_JP,
            bytes.iter().map(|byte| byte | 0x80).collect::<Vec<u8>>(),
        ),
        KanjiEncoding::ShiftJis => (SHIFT_JIS, bytes.to_vec()),
        KanjiEncoding::Gb18030 => (GB18030, bytes.to_vec()),
        KanjiEncoding::Big5 => (BIG5, bytes.to_vec()),
        KanjiEncoding::EucKr => (EUC_KR, bytes.to_vec()),
    };
    encoding.decode_without_bom_handling(&bytes).0.into_owned()
}

/// A stretch of a line read the same way.
enum Piece {
    /// Bytes below 0x80, read in the code page.
    Narrow(std::ops::Range<usize>),
    /// Bytes read with the two byte encoding.
    Wide(std::ops::Range<usize>),
    /// A character the program drew with `FS 2`, as it is shown in the text.
    Drawn(char),
}

/// The character that shows a Kanji drawn with `FS 2`, for the two bytes of its code. A code a
/// program may draw but has not is a space, as the printer prints it.
fn drawn_glyph(two_byte: &TwoByte, unit: &[u8]) -> Option<char> {
    let &[first, second] = unit else {
        return None;
    };
    let glyph = UserKanji::glyph([first, second])?;
    Some(if two_byte.drawn.contains(&[first, second]) {
        glyph
    } else {
        ' '
    })
}

/// Cuts a line into pieces. The second byte of a two byte character can look like ASCII, so the
/// line is read in units of the encoding and not byte by byte.
fn pieces(two_byte: &TwoByte, bytes: &[u8]) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let length = kanji_length(two_byte.encoding, &bytes[index..]);
        let range = index..index + length;
        index += length;
        let piece = if let Some(glyph) = drawn_glyph(two_byte, &bytes[range.clone()]) {
            Piece::Drawn(glyph)
        } else if length == 1 && bytes[range.start] < 0x80 {
            Piece::Narrow(range)
        } else {
            Piece::Wide(range)
        };
        match (pieces.last_mut(), piece) {
            (Some(Piece::Narrow(last)), Piece::Narrow(next))
            | (Some(Piece::Wide(last)), Piece::Wide(next)) => last.end = next.end,
            (_, piece) => pieces.push(piece),
        }
    }
    pieces
}

/// Reads the bytes of a printed line.
pub(crate) fn decode_line(context: &TextContext, bytes: &[u8]) -> String {
    if context.utf8 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let table = table(context.page.unwrap_or("CP1252"));
    let Some(two_byte) = &context.kanji else {
        return decode_single(context, &table, bytes);
    };

    let mut text = String::new();
    for piece in pieces(two_byte, bytes) {
        match piece {
            Piece::Narrow(range) => text.push_str(&decode_single(context, &table, &bytes[range])),
            Piece::Wide(range) => text.push_str(&decode_wide(two_byte.encoding, &bytes[range])),
            Piece::Drawn(glyph) => text.push(glyph),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(page: &str) -> TextContext<'_> {
        TextContext {
            page: Some(page),
            international: 0,
            user_characters: None,
            kanji: None,
            utf8: false,
        }
    }

    fn read(page: &str, bytes: &[u8]) -> String {
        decode_line(&context(page), bytes)
    }

    #[test]
    fn ascii_reads_the_same_in_every_page() {
        for page in ["CP437", "CP850", "CP1252", "CP864", "CP1256", "Nothing"] {
            assert_eq!(read(page, b"Total 12.50"), "Total 12.50", "page {page}");
        }
    }

    #[test]
    fn cp437_has_accents_and_box_lines() {
        assert_eq!(read("CP437", &[0x82, 0x81, 0xa4, 0x87, 0xa8]), "éüñç¿");
        assert_eq!(read("CP437", &[0xc4, 0xcd]), "─═");
    }

    #[test]
    fn windows_1252_has_the_euro_and_dashes() {
        assert_eq!(read("CP1252", &[0x80, 0x97, 0x8c]), "€—Œ");
    }

    #[test]
    fn windows_1256_has_arabic() {
        assert_eq!(read("CP1256", &[0xe3, 0xd1, 0xcd, 0xc8, 0xc7]), "مرحبا");
    }

    #[test]
    fn cp864_has_arabic_in_its_own_places() {
        assert_ne!(read("CP864", &[0xc1]), read("CP1256", &[0xc1]));
    }

    #[test]
    fn a_page_with_no_name_is_read_as_windows_1252() {
        let mut context = context("CP437");
        context.page = None;
        assert_eq!(decode_line(&context, &[0x80]), "€");
        assert_eq!(read("CP3012", &[0x80]), "€");
    }

    #[test]
    fn katakana_is_the_japanese_half_of_shift_jis() {
        assert_eq!(read("CP932", &[0xb1, 0xb2, b'A']), "ｱｲA");
    }

    #[test]
    fn more_pages_than_the_ibm_ones_are_read() {
        assert_eq!(read("ISO_8859-15", &[0xa4]), "€");
        assert_eq!(read("CP858", &[0xd5]), "€");
        assert_eq!(read("CP866", &[0x80]), "А");
        assert_eq!(read("CP1251", &[0xc0]), "А");
    }

    #[test]
    fn an_international_set_replaces_ascii_characters() {
        let mut context = context("CP437");
        context.international = 1;
        assert_eq!(decode_line(&context, b"#$@[\\]^`{|}~"), "#$à°ç§^`éùè¨");
        context.international = 2;
        assert_eq!(decode_line(&context, b"[\\]{|}~"), "ÄÖÜäöüß");
        context.international = 3;
        assert_eq!(decode_line(&context, b"#"), "£");
        context.international = 0;
        assert_eq!(decode_line(&context, b"#[\\]"), "#[\\]");
        context.international = 200;
        assert_eq!(decode_line(&context, b"#[\\]"), "#[\\]");
    }

    #[test]
    fn drawn_characters_are_marked_in_the_text() {
        let characters: BTreeSet<u8> = BTreeSet::from(*b"A");
        let mut context = context("CP437");
        context.user_characters = Some(&characters);
        assert_eq!(decode_line(&context, b"xAy"), "x\u{e041}y");
    }

    #[test]
    fn two_byte_characters_are_read_when_they_are_on() {
        let mut context = context("CP437");
        let drawn = BTreeSet::new();
        let two_byte = |encoding| {
            Some(TwoByte {
                encoding,
                drawn: &drawn,
            })
        };
        context.kanji = two_byte(KanjiEncoding::ShiftJis);
        // 日本 in Shift JIS, with ASCII around it, and a second byte that looks like ASCII.
        assert_eq!(decode_line(&context, b"a\x93\xfa\x96\x7bb"), "a日本b");
        context.kanji = two_byte(KanjiEncoding::Gb18030);
        assert_eq!(decode_line(&context, &[0xd6, 0xd0, b'!']), "中!");
        context.kanji = two_byte(KanjiEncoding::Big5);
        assert_eq!(decode_line(&context, &[0xa4, 0xa4]), "中");
        context.kanji = two_byte(KanjiEncoding::EucKr);
        assert_eq!(decode_line(&context, &[0xb0, 0xa1]), "가");
    }

    #[test]
    fn jis_code_is_read_in_pairs_of_bytes_below_0x7f() {
        let drawn = BTreeSet::new();
        let mut context = context("CP437");
        context.kanji = Some(TwoByte {
            encoding: KanjiEncoding::Jis,
            drawn: &drawn,
        });
        // 日本 in JIS, then two ASCII letters that are read as a pair, as the printer does.
        assert_eq!(decode_line(&context, &[0x46, 0x7c, 0x4b, 0x5c]), "日本");
        assert_eq!(decode_line(&context, b"AB"), "疎");
        // A lone byte, or one that is not a JIS byte, is read as a single character.
        assert_eq!(decode_line(&context, b"A"), "A");
        assert_eq!(decode_line(&context, &[0x20, 0x21, 0x21]), " \u{3000}");
    }

    #[test]
    fn drawn_kanji_show_as_private_characters_and_the_others_as_spaces() {
        let drawn = BTreeSet::from([[0x77, 0x21]]);
        let mut context = context("CP437");
        context.kanji = Some(TwoByte {
            encoding: KanjiEncoding::Jis,
            drawn: &drawn,
        });
        assert_eq!(
            decode_line(&context, &[0x77, 0x21, 0x77, 0x22]),
            "\u{e100} "
        );
        let drawn = BTreeSet::from([[0xec, 0x80]]);
        context.kanji = Some(TwoByte {
            encoding: KanjiEncoding::ShiftJis,
            drawn: &drawn,
        });
        assert_eq!(
            decode_line(&context, &[0xec, 0x80, 0xec, 0x40]),
            "\u{e13f} "
        );
    }

    #[test]
    fn utf8_text_is_read_as_utf8() {
        let mut context = context("CP437");
        context.utf8 = true;
        assert_eq!(decode_line(&context, "é中".as_bytes()), "é中");
    }
}
