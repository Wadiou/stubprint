//! Every fixture stream is drawn, as a picture and as text, and so is noise.

use std::path::PathBuf;

use stubprint_core::{Dialect, Printer, Profile, Receipt};
use stubprint_render::{picture, png, text_of, Options};

fn streams() -> Vec<(String, Dialect, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/out/escpos");
    let mut streams = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("run fixtures/generate/generate.py first") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "bin") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let expected: serde_json::Value = serde_json::from_slice(
                &std::fs::read(dir.join(format!("{name}.expected.json"))).unwrap(),
            )
            .unwrap();
            let dialect = expected["dialect"]
                .as_str()
                .and_then(Dialect::from_name)
                .unwrap_or(Dialect::EscPos);
            streams.push((name, dialect, std::fs::read(&path).unwrap()));
        }
    }
    streams.sort_by(|a, b| a.0.cmp(&b.0));
    streams
}

fn receipts(dialect: Dialect, bytes: &[u8]) -> Vec<Receipt> {
    let mut printer = Printer::with_dialect(dialect, Profile::generic());
    let mut all = printer.push(bytes);
    all.extend(printer.finish());
    all
}

#[test]
fn every_receipt_of_every_fixture_is_drawn_as_wide_as_the_paper_with_something_on_it() {
    let options = Options::default();
    let mut drawn = 0;
    for (name, dialect, bytes) in streams() {
        for receipt in receipts(dialect, &bytes) {
            let canvas = picture(&receipt, &options);
            assert_eq!(canvas.width, options.width, "{name}");
            assert!(canvas.height > 16, "{name}: {} dots tall", canvas.height);
            // This one moves the print position 17,732 dots along, far past the edge of the paper,
            // so what it prints is off the paper.
            let off_the_paper = name == "edge-length-framed-unknown";
            assert!(
                canvas.pixels.iter().any(|dot| *dot < 128) != off_the_paper,
                "{name}: nothing is drawn, or something is where it cannot be"
            );
            let file = png(&receipt, &options).unwrap();
            assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n", "{name}");
            let text = text_of(&receipt, &options);
            assert!(text.ends_with('\n') && !text.trim().is_empty(), "{name}");
            drawn += 1;
        }
    }
    assert!(drawn >= 100, "only {drawn} receipts");
}

#[test]
fn the_text_of_a_fixture_holds_the_lines_its_spec_expects() {
    // Native receipts are laid out by the generator, so the lines of the picture's text are the
    // lines the spec lists, in order, ignoring the blank ones.
    let options = Options::default();
    let (_, dialect, bytes) = streams()
        .into_iter()
        .find(|(name, ..)| name == "receipt-native-en-80")
        .unwrap();
    let text = text_of(&receipts(dialect, &bytes)[0], &options);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines[0], "Green Market");
    assert!(lines.contains(&"Whole milk 1 L"));
    assert!(lines
        .iter()
        .any(|l| l.starts_with("TOTAL") && l.ends_with("460.50 USD")));
}

#[test]
fn noise_is_drawn_without_stopping() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let noise: Vec<u8> = (0..300_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect();
    let options = Options::default();
    for dialect in [Dialect::EscPos, Dialect::StarLine, Dialect::StarPrnt] {
        for receipt in receipts(dialect, &noise) {
            let canvas = picture(&receipt, &options);
            assert_eq!(canvas.width, options.width);
            let _ = text_of(&receipt, &options);
        }
    }
}
