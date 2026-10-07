//! Draws the bytes of a file as the paper a printer would print, to look at while developing:
//!
//!     cargo run -p stubprint-render --example render -- stream.bin out.png [dialect [profile]]
//!
//! Without a profile the paper is 80 mm with 48 columns, which is what the fixtures are made for.
//! Instead of a profile, `384:32,42` is a paper 384 dots wide with 32 and 42 columns in its fonts.

use stubprint_core::{Dialect, Printer, Profile};
use stubprint_render::{png, text_of, Options};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output, rest @ ..] = args.as_slice() else {
        eprintln!("usage: render <stream> <out.png> [dialect]");
        std::process::exit(2);
    };
    let dialect = rest
        .first()
        .and_then(|name| Dialect::from_name(name))
        .unwrap_or(Dialect::EscPos);
    let bytes = std::fs::read(input).expect("the stream can be read");
    let paper = rest.get(1).and_then(|text| {
        let (width, columns) = text.split_once(':')?;
        Some(Options {
            width: width.parse().ok()?,
            columns: columns.split(',').filter_map(|c| c.parse().ok()).collect(),
            scale: 1,
        })
    });
    let profile = rest
        .get(1)
        .and_then(|name| Profile::named(name))
        .unwrap_or_else(Profile::generic);
    let options = match (paper, rest.len() > 1) {
        (Some(options), _) => options,
        (None, true) => Options::for_profile(&profile),
        (None, false) => Options::default(),
    };
    let mut printer = Printer::with_dialect(dialect, profile);
    let mut receipts = printer.push(&bytes);
    receipts.extend(printer.finish());
    for (index, receipt) in receipts.iter().enumerate() {
        let name = if receipts.len() == 1 {
            output.clone()
        } else {
            output.replace(".png", &format!("-{}.png", index + 1))
        };
        std::fs::write(&name, png(receipt, &options).expect("png")).expect("written");
        println!("{name}\n{}", text_of(receipt, &options));
    }
}
