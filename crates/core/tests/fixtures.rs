//! Decodes every generated fixture and checks it against what its spec says it must decode to.
//!
//! The fixtures come from `fixtures/specs` and are built with `python generate.py` in
//! `fixtures/generate`. Each stream is decoded whole, one byte at a time, and cut in two at every
//! position, and the result must not depend on how it was cut.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use receipt::{
    Dialect, Event, Fault, Page, Paper, Position, Printer, PrinterStatus, Profile, Receipt,
};
use serde_json::{json, Value};

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
                "No fixtures in {}. Run `python generate.py` in fixtures/generate, see the README.",
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

/// What came out of a printer: the receipts, and the bytes it answered with.
struct Decoded {
    receipts: Vec<Receipt>,
    replies: Vec<u8>,
}

fn status_from(spec: &Value) -> PrinterStatus {
    let flag = |key: &str, default: bool| spec.get(key).map_or(default, |v| v.as_bool().unwrap());
    PrinterStatus {
        online: flag("online", true),
        cover_open: flag("coverOpen", false),
        paper: match spec.get("paper").and_then(Value::as_str) {
            None | Some("adequate") => Paper::Adequate,
            Some("nearEnd") => Paper::NearEnd,
            Some("end") => Paper::End,
            Some(other) => panic!("unknown paper status {other}"),
        },
        drawer_open: flag("drawerOpen", false),
        feed_button: flag("feedButton", false),
        fault: match spec.get("fault").and_then(Value::as_str) {
            None => None,
            Some("cutter") => Some(Fault::Cutter),
            Some("unrecoverable") => Some(Fault::Unrecoverable),
            Some("recoverable") => Some(Fault::Recoverable),
            Some(other) => panic!("unknown fault {other}"),
        },
    }
}

/// A printer of the model the spec names, set to the language it names, in the state it says.
fn printer_for(fixture: &Fixture) -> Printer {
    let text = |key: &str| fixture.expected.get(key).and_then(Value::as_str);
    let dialect = text("dialect").map_or(Dialect::EscPos, |name| {
        Dialect::from_name(name).unwrap_or_else(|| panic!("no command language {name}"))
    });
    let profile = text("profile").map_or_else(Profile::generic, |name| {
        Profile::named(name).unwrap_or_else(|| panic!("no printer profile {name}"))
    });
    let mut printer = Printer::with_dialect(dialect, profile);
    if let Some(status) = fixture.expected.get("status") {
        printer.set_status(status_from(status));
    }
    printer
}

fn decode_in_pieces(fixture: &Fixture, pieces: &[usize]) -> Decoded {
    let bytes = &fixture.bytes;
    let mut printer = printer_for(fixture);
    let mut receipts = Vec::new();
    let mut replies = printer.take_replies();
    let mut start = 0;
    for &end in pieces.iter().chain(std::iter::once(&bytes.len())) {
        receipts.extend(printer.push(&bytes[start..end]));
        replies.extend(printer.take_replies());
        start = end;
    }
    receipts.extend(printer.finish());
    replies.extend(printer.take_replies());
    Decoded { receipts, replies }
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

/// The keys of the events that a spec checks as a list, in the order they happened. See `extra`.
const EVENT_KEYS: &[&str] = &[
    "beeps",
    "positions",
    "margins",
    "printWidths",
    "motionUnits",
    "tabStops",
    "lineSpacings",
    "feeds",
    "densities",
    "graphicsDensities",
    "symbols",
    "userCharacters",
    "userKanji",
    "page",
];

/// The other keys a spec may use. A key in neither list fails, so nothing is quietly left
/// unchecked.
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
    "pulses",
    "unknown",
    "images",
    "replies",
    "profile",
    "dialect",
    "status",
];

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A picture as rows of `#` for black and `.` for white dots, which a spec can write by hand.
fn picture_json(bitmap: &receipt::Bitmap) -> Value {
    let rows: Vec<String> = (0..bitmap.height())
        .map(|y| {
            (0..bitmap.width())
                .map(|x| if bitmap.is_black(x, y) { '#' } else { '.' })
                .collect()
        })
        .collect();
    json!({ "width": bitmap.width(), "height": bitmap.height(), "rows": rows })
}

fn position_json(position: &Position) -> Value {
    match position {
        Position::Absolute(value) => json!({ "absolute": value }),
        Position::Relative(value) => json!({ "relative": value }),
    }
}

/// The events that a spec checks as a list of JSON, by the key it uses for them.
fn extra(event: &Event) -> Option<(&'static str, Value)> {
    use receipt::{FeedAmount, LineSpacing};
    Some(match event {
        Event::Beep { times, duration } => {
            ("beeps", json!({ "times": times, "duration": duration }))
        }
        Event::Position(position) => ("positions", position_json(position)),
        Event::Margin(value) => ("margins", json!(value)),
        Event::PrintWidth(value) => ("printWidths", json!(value)),
        Event::MotionUnits { x, y } => ("motionUnits", json!({ "x": x, "y": y })),
        Event::TabStops(stops) => ("tabStops", json!(stops)),
        Event::LineSpacing(spacing) => (
            "lineSpacings",
            match spacing {
                LineSpacing::Default => json!("default"),
                LineSpacing::Units(n) => json!({ "units": n }),
                LineSpacing::Sixtieths(n) => json!({ "sixtieths": n }),
                LineSpacing::ThreeSixtieths(n) => json!({ "threeSixtieths": n }),
                LineSpacing::Dots(n) => json!({ "dots": n }),
            },
        ),
        Event::Feed(feed) => {
            let mut value = match feed.amount {
                FeedAmount::Lines(n) => json!({ "lines": n }),
                FeedAmount::Units(n) => json!({ "units": n }),
                FeedAmount::Page => json!("page"),
            };
            if feed.reverse {
                value["reverse"] = json!(true);
            }
            ("feeds", value)
        }
        Event::Density(value) => ("densities", json!(value)),
        Event::Symbol(symbol) => (
            "symbols",
            json!({
                "kind": symbol.kind.name(),
                "data": String::from_utf8_lossy(&symbol.data),
                "settings": symbol
                    .settings
                    .iter()
                    .map(|(function, bytes)| json!({ "function": function, "bytes": hex(bytes) }))
                    .collect::<Vec<_>>(),
            }),
        ),
        Event::DefineCharacter(character) => (
            "userCharacters",
            json!({ "define": {
                "code": character.code,
                "width": character.width,
                "columnBytes": character.column_bytes,
                "data": hex(&character.data),
            } }),
        ),
        Event::DeleteCharacter(code) => ("userCharacters", json!({ "delete": code })),
        Event::DefineKanji(kanji) => (
            "userKanji",
            json!({
                "code": hex(&kanji.code),
                "width": kanji.width,
                "height": kanji.height,
                "data": hex(&kanji.data),
            }),
        ),
        Event::GraphicsDensity(dpi) => ("graphicsDensities", json!(dpi)),
        Event::UserCharacters(on) => ("userCharacters", json!({ "use": on })),
        Event::Page(page) => (
            "page",
            match page {
                Page::Enter => json!("enter"),
                Page::Standard => json!("standard"),
                Page::Print => json!("print"),
                Page::Cancel => json!("cancel"),
                Page::Direction(direction) => json!({ "direction": direction }),
                Page::Area {
                    x,
                    y,
                    width,
                    height,
                } => json!({ "area": { "x": x, "y": y, "width": width, "height": height } }),
                Page::Vertical(position) => json!({ "vertical": position_json(position) }),
            },
        ),
        _ => return None,
    })
}

/// Compares what was decoded with what the spec expects and returns everything that differs.
fn problems(fixture: &Fixture, decoded: &Decoded) -> Vec<String> {
    let receipts = &decoded.receipts[..];
    let expected = fixture.expected.as_object().unwrap();
    let mut problems = Vec::new();
    let mut check = |ok: bool, message: String| {
        if !ok {
            problems.push(message);
        }
    };

    for key in expected.keys() {
        check(
            KNOWN_KEYS.contains(&key.as_str()) || EVENT_KEYS.contains(&key.as_str()),
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
                        && style.overline == flag("overline").unwrap_or(false)
                        && style.invert == flag("invert").unwrap_or(false)
                        && u64::from(style.width) == number("width", 1)
                        && u64::from(style.height) == number("height", 1)
                        && u64::from(style.font) == number("font", 0)
                        && style.double_strike == flag("doubleStrike").unwrap_or(false)
                        && style.upside_down == flag("upsideDown").unwrap_or(false)
                        && style.rotated == flag("rotated").unwrap_or(false)
                        && style.smooth == flag("smooth").unwrap_or(false)
                        && u64::from(style.color) == number("color", 0)
                        && u64::from(style.spacing) == number("spacing", 0)
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
                && want.get("text").is_none_or(|v| got.text.name() == v)
                && want
                    .get("font")
                    .is_none_or(|v| u64::from(got.font) == v.as_u64().unwrap());
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
        let got: Vec<&receipt::Pdf417> = events(receipts)
            .filter_map(|event| match event {
                Event::Pdf417(code) => Some(code),
                _ => None,
            })
            .collect();
        let want = codes.as_array().unwrap();
        check(
            got.len() == want.len(),
            format!("pdf417: expected {}, got {}", want.len(), got.len()),
        );
        for (got, want) in got.iter().zip(want) {
            let number = |key: &str, value: u8| {
                want.get(key)
                    .is_none_or(|v| u64::from(value) == v.as_u64().unwrap())
            };
            let same = got.data == want["data"].as_str().unwrap()
                && number("columns", got.columns)
                && number("rows", got.rows)
                && number("width", got.width)
                && number("height", got.height)
                && number("levelMode", got.level_mode)
                && number("level", got.level)
                && want
                    .get("truncated")
                    .is_none_or(|v| got.truncated == v.as_bool().unwrap());
            check(same, format!("pdf417: expected {want}, got {got:?}"));
        }
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

    // The events that have no key of their own above, by the key of the spec.
    let mut extras: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for event in events(receipts) {
        if let Some((key, value)) = extra(event) {
            extras.entry(key).or_default().push(value);
        }
    }
    for key in EVENT_KEYS {
        if let Some(want) = expected.get(*key) {
            let got = Value::Array(extras.get(key).cloned().unwrap_or_default());
            check(&got == want, format!("{key}: expected {want}, got {got}"));
        }
    }

    if let Some(want) = expected.get("images") {
        let got = Value::Array(
            events(receipts)
                .filter_map(|event| match event {
                    Event::Image(bitmap) => Some(picture_json(bitmap)),
                    _ => None,
                })
                .collect(),
        );
        check(&got == want, format!("images: expected {want}, got {got}"));
    }

    if let Some(want) = expected.get("replies") {
        let got = hex(&decoded.replies);
        check(
            got == want.as_str().unwrap(),
            format!("replies: expected {want}, got \"{got}\""),
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

fn assert_fixture(fixture: &Fixture, decoded: &Decoded, how: &str) {
    let problems = problems(fixture, decoded);
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
        assert_fixture(&fixture, &decode_in_pieces(&fixture, &[]), "whole");
    }
}

#[test]
fn every_fixture_decodes_the_same_one_byte_at_a_time() {
    for fixture in load() {
        let cuts: Vec<usize> = (1..fixture.bytes.len()).collect();
        assert_fixture(
            &fixture,
            &decode_in_pieces(&fixture, &cuts),
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
                &decode_in_pieces(&fixture, &[position]),
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
