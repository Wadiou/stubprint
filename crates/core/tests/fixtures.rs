//! Decodes every generated fixture and checks it against what its spec says it must decode to.
//!
//! The fixtures come from `fixtures/specs` and are built with `npm run generate` in
//! `fixtures/generate`. Each stream is decoded whole, one byte at a time, and cut in two at every
//! position, and the result must not depend on how it was cut.

use std::fs;
use std::path::PathBuf;

use receipt::{Event, Printer, Receipt};
use serde_json::Value;

struct Fixture {
    name: String,
    bytes: Vec<u8>,
    expected: Value,
    bitmap: Option<Vec<u8>>,
}

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/out/escpos")
}

fn load() -> Vec<Fixture> {
    let dir = out_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|_| {
            panic!(
                "No fixtures in {}. Run `npm install` and `npm run generate` in fixtures/generate.",
                dir.display()
            )
        })
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".expected.json").map(str::to_owned)
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "the fixtures folder has no fixtures");

    names
        .into_iter()
        .map(|name| Fixture {
            bytes: fs::read(dir.join(format!("{name}.bin"))).unwrap(),
            expected: serde_json::from_slice(
                &fs::read(dir.join(format!("{name}.expected.json"))).unwrap(),
            )
            .unwrap(),
            bitmap: fs::read(dir.join(format!("{name}.bitmap"))).ok(),
            name,
        })
        .collect()
}

fn decode_in_pieces(bytes: &[u8], pieces: &[usize]) -> Vec<Receipt> {
    let mut printer = Printer::new();
    let mut receipts = Vec::new();
    let mut start = 0;
    for &end in pieces.iter().chain(std::iter::once(&bytes.len())) {
        receipts.extend(printer.push(&bytes[start..end]));
        start = end;
    }
    receipts.extend(printer.finish());
    receipts
}

fn events(receipts: &[Receipt]) -> impl Iterator<Item = &Event> {
    receipts.iter().flat_map(|receipt| receipt.events())
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_owned())
        .collect()
}

/// Every key a spec may use. A key not listed here fails, so nothing is quietly left unchecked.
const KNOWN_KEYS: &[&str] = &[
    "receipts",
    "receiptTexts",
    "texts",
    "boldTexts",
    "styles",
    "codepage",
    "columns",
    "barcodes",
    "qrs",
    "pdf417",
    "image",
    "rasterBands",
    "columnStrips",
    "cuts",
    "fonts",
    "aligns",
    "feeds",
    "pulses",
    "unknown",
];

/// Compares what was decoded with what the spec expects and returns everything that differs.
fn problems(fixture: &Fixture, receipts: &[Receipt]) -> Vec<String> {
    let expected = fixture.expected.as_object().unwrap();
    let mut problems = Vec::new();
    let mut check = |ok: bool, message: String| {
        if !ok {
            problems.push(message);
        }
    };

    for key in expected.keys() {
        check(
            KNOWN_KEYS.contains(&key.as_str()),
            format!("the spec uses an unchecked key: {key}"),
        );
    }

    if let Some(count) = expected.get("receipts") {
        check(
            receipts.len() as u64 == count.as_u64().unwrap(),
            format!("receipts: expected {count}, got {}", receipts.len()),
        );
    }

    if let Some(groups) = expected.get("receiptTexts") {
        let got: Vec<Vec<&str>> = receipts.iter().map(Receipt::texts).collect();
        let want: Vec<Vec<String>> = groups.as_array().unwrap().iter().map(strings).collect();
        check(
            got == want,
            format!("receiptTexts: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(texts) = expected.get("texts") {
        let got: Vec<&str> = receipts.iter().flat_map(Receipt::texts).collect();
        let want = strings(texts);
        check(
            got == want,
            format!("texts: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(texts) = expected.get("boldTexts") {
        let got: Vec<&str> = events(receipts)
            .filter_map(|event| match event {
                Event::Text { text, style, .. } if style.bold && !text.is_empty() => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        let want = strings(texts);
        check(
            got == want,
            format!("boldTexts: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(styles) = expected.get("styles") {
        for want in styles.as_array().unwrap() {
            let text = want["text"].as_str().unwrap();
            let found = events(receipts).any(|event| match event {
                Event::Text {
                    text: got, style, ..
                } if got == text => {
                    let flag = |key: &str| want.get(key).map(|v| v.as_bool() == Some(true));
                    let number = |key: &str, default: u64| {
                        want.get(key).map_or(default, |v| v.as_u64().unwrap())
                    };
                    style.bold == flag("bold").unwrap_or(false)
                        && (style.underline != 0) == flag("underline").unwrap_or(false)
                        && style.invert == flag("invert").unwrap_or(false)
                        && u64::from(style.width) == number("width", 1)
                        && u64::from(style.height) == number("height", 1)
                }
                _ => false,
            });
            check(
                found,
                format!("styles: no text \"{text}\" printed with {want}"),
            );
        }
    }

    if let Some(barcodes) = expected.get("barcodes") {
        let got: Vec<&receipt::Barcode> = events(receipts)
            .filter_map(|event| match event {
                Event::Barcode(barcode) => Some(barcode),
                _ => None,
            })
            .collect();
        let want = barcodes.as_array().unwrap();
        check(
            got.len() == want.len(),
            format!("barcodes: expected {}, got {}", want.len(), got.len()),
        );
        for (got, want) in got.iter().zip(want) {
            let same = got.symbology.name() == want["symbology"]
                && got.data == want["data"].as_str().unwrap()
                && want
                    .get("height")
                    .is_none_or(|v| u64::from(got.height) == v.as_u64().unwrap())
                && want
                    .get("width")
                    .is_none_or(|v| u64::from(got.width) == v.as_u64().unwrap())
                && want.get("text").is_none_or(|v| got.text.name() == v);
            check(same, format!("barcode: expected {want}, got {got:?}"));
        }
    }

    if let Some(qrs) = expected.get("qrs") {
        let got: Vec<&receipt::Qr> = events(receipts)
            .filter_map(|event| match event {
                Event::Qr(qr) => Some(qr),
                _ => None,
            })
            .collect();
        let want = qrs.as_array().unwrap();
        check(
            got.len() == want.len(),
            format!("qrs: expected {}, got {}", want.len(), got.len()),
        );
        for (got, want) in got.iter().zip(want) {
            let same = got.data == want["data"].as_str().unwrap()
                && u64::from(got.model) == want["model"].as_u64().unwrap()
                && u64::from(got.size) == want["size"].as_u64().unwrap()
                && got.level.name() == want["level"];
            check(same, format!("qr: expected {want}, got {got:?}"));
        }
    }

    if let Some(codes) = expected.get("pdf417") {
        let got: Vec<&str> = events(receipts)
            .filter_map(|event| match event {
                Event::Pdf417(code) => Some(code.data.as_str()),
                _ => None,
            })
            .collect();
        let want: Vec<String> = codes
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["data"].as_str().unwrap().to_owned())
            .collect();
        check(
            got == want,
            format!("pdf417: expected {want:?}, got {got:?}"),
        );
    }

    let bands = events(receipts)
        .filter(|event| matches!(event, Event::Image(_)))
        .count();
    for key in ["rasterBands", "columnStrips"] {
        if let Some(count) = expected.get(key) {
            check(
                bands as u64 == count.as_u64().unwrap(),
                format!("{key}: expected {count}, got {bands}"),
            );
        }
    }

    if let Some(image) = expected.get("image") {
        let picture = receipts.iter().find_map(Receipt::image);
        match picture {
            None => check(false, "image: nothing was printed".to_owned()),
            Some(picture) => {
                check(
                    u64::from(picture.width()) == image["width"].as_u64().unwrap()
                        && u64::from(picture.height()) == image["height"].as_u64().unwrap(),
                    format!(
                        "image: expected {}x{}, got {}x{}",
                        image["width"],
                        image["height"],
                        picture.width(),
                        picture.height()
                    ),
                );
                if let Some(bitmap) = &fixture.bitmap {
                    check(
                        picture.data() == bitmap.as_slice(),
                        "image: the dots differ from the expected bitmap".to_owned(),
                    );
                }
            }
        }
    }

    if let Some(cuts) = expected.get("cuts") {
        let got: Vec<&str> = events(receipts)
            .filter_map(|event| match event {
                Event::Cut(mode) => Some(mode.name()),
                _ => None,
            })
            .collect();
        let want = strings(cuts);
        check(got == want, format!("cuts: expected {want:?}, got {got:?}"));
    }

    if let Some(fonts) = expected.get("fonts") {
        let got: Vec<u64> = events(receipts)
            .filter_map(|event| match event {
                Event::Font(font) => Some(u64::from(*font)),
                _ => None,
            })
            .collect();
        let want: Vec<u64> = fonts
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_u64().unwrap())
            .collect();
        check(
            got == want,
            format!("fonts: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(aligns) = expected.get("aligns") {
        let got: Vec<&str> = events(receipts)
            .filter_map(|event| match event {
                Event::Align(align) => Some(align.name()),
                _ => None,
            })
            .collect();
        let want = strings(aligns);
        check(
            got == want,
            format!("aligns: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(feeds) = expected.get("feeds") {
        let got: Vec<u64> = events(receipts)
            .filter_map(|event| match event {
                Event::Feed(lines) => Some(u64::from(*lines)),
                _ => None,
            })
            .collect();
        let want: Vec<u64> = feeds
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_u64().unwrap())
            .collect();
        check(
            got == want,
            format!("feeds: expected {want:?}, got {got:?}"),
        );
    }

    if let Some(pulses) = expected.get("pulses") {
        let got: Vec<(u64, u64, u64)> = events(receipts)
            .filter_map(|event| match event {
                Event::Pulse(pulse) => Some((
                    u64::from(pulse.pin),
                    u64::from(pulse.on_ms),
                    u64::from(pulse.off_ms),
                )),
                _ => None,
            })
            .collect();
        let want: Vec<(u64, u64, u64)> = pulses
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["pin"].as_u64().unwrap(),
                    p["onMs"].as_u64().unwrap(),
                    p["offMs"].as_u64().unwrap(),
                )
            })
            .collect();
        check(
            got == want,
            format!("pulses: expected {want:?}, got {got:?}"),
        );
    }

    let unknown = events(receipts)
        .filter(|event| matches!(event, Event::Unknown { .. }))
        .count();
    let want_unknown = expected.get("unknown").map_or(0, |v| v.as_u64().unwrap());
    check(
        unknown as u64 == want_unknown,
        format!("unknown commands: expected {want_unknown}, got {unknown}"),
    );

    problems
}

fn assert_fixture(fixture: &Fixture, receipts: &[Receipt], how: &str) {
    let problems = problems(fixture, receipts);
    assert!(
        problems.is_empty(),
        "{} ({how}):\n  {}",
        fixture.name,
        problems.join("\n  ")
    );
}

#[test]
fn every_fixture_decodes_as_its_spec_says() {
    for fixture in load() {
        assert_fixture(&fixture, &decode_in_pieces(&fixture.bytes, &[]), "whole");
    }
}

#[test]
fn every_fixture_decodes_the_same_one_byte_at_a_time() {
    for fixture in load() {
        let cuts: Vec<usize> = (1..fixture.bytes.len()).collect();
        assert_fixture(
            &fixture,
            &decode_in_pieces(&fixture.bytes, &cuts),
            "one byte at a time",
        );
    }
}

/// Positions worth cutting at: all of them for a small stream, and for a large one the start,
/// the end and a spread through the middle, since the middle of a picture is only data.
fn cut_positions(length: usize) -> Vec<usize> {
    if length <= 2_000 {
        return (1..length).collect();
    }
    (1..300)
        .chain((300..length - 300).step_by(997))
        .chain(length - 300..length)
        .collect()
}

#[test]
fn every_fixture_decodes_the_same_cut_in_two_anywhere() {
    for fixture in load() {
        for position in cut_positions(fixture.bytes.len()) {
            assert_fixture(
                &fixture,
                &decode_in_pieces(&fixture.bytes, &[position]),
                &format!("cut at byte {position}"),
            );
        }
    }
}

#[test]
fn random_bytes_never_panic() {
    // A small xorshift, so the same bytes are tried every run.
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..300 {
        let length = (next() % 400) as usize;
        let bytes: Vec<u8> = (0..length).map(|_| next() as u8).collect();
        let cut = if length > 0 {
            (next() as usize) % length
        } else {
            0
        };
        let mut printer = Printer::new();
        printer.push(&bytes[..cut]);
        printer.push(&bytes[cut..]);
        printer.finish();
    }
}

#[test]
fn a_command_is_held_until_the_rest_arrives() {
    let fixture = load()
        .into_iter()
        .find(|f| f.name == "image-384x96")
        .unwrap();
    let mut printer = Printer::new();
    assert!(printer.push(&fixture.bytes[..20]).is_empty());
    let receipts = {
        let mut receipts = printer.push(&fixture.bytes[20..]);
        receipts.extend(printer.finish());
        receipts
    };
    assert_eq!(receipts.len(), 1);
}
