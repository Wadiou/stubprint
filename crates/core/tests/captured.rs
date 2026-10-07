//! Decodes the streams other people's encoders wrote, kept in `fixtures/captured`.
//!
//! No spec says what these must decode to, so the checks are the ones that need no expectation:
//! no command is left unknown, no byte is left waiting, and the events do not depend on where the
//! stream was cut. A command the decoder does not read yet is listed in `KNOWN_GAPS`, with the
//! stage of `docs/COVERAGE-PLAN.md` that closes it. The list only shrinks: a command that is
//! listed but no longer unknown fails the test, so the entry has to be removed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use stubprint_core::{Decoder, Event};

/// Commands the decoder reports as unknown today. The bytes are the start of the command.
const KNOWN_GAPS: &[(&[u8], &str)] = &[];

struct Stream {
    name: String,
    bytes: Vec<u8>,
}

fn captured_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captured")
}

fn collect(dir: &Path, found: &mut Vec<Stream>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|_| panic!("no captured streams in {}", dir.display()))
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("bin" | "raw")
        ) {
            found.push(Stream {
                name: path
                    .strip_prefix(captured_dir())
                    .unwrap()
                    .display()
                    .to_string(),
                bytes: fs::read(&path).unwrap(),
            });
        }
    }
}

fn streams() -> Vec<Stream> {
    let mut found = Vec::new();
    collect(&captured_dir(), &mut found);
    assert!(!found.is_empty(), "the captured folder has no streams");
    found
}

/// The events of a stream cut into the given pieces, and how many bytes were left waiting.
fn decode(bytes: &[u8], cuts: &[usize]) -> (Vec<Event>, usize) {
    let mut decoder = Decoder::new();
    let mut events = Vec::new();
    let mut start = 0;
    for &end in cuts.iter().chain(std::iter::once(&bytes.len())) {
        events.extend(decoder.push(&bytes[start..end]));
        start = end;
    }
    events.extend(decoder.finish());
    (events, decoder.pending_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn known_gap(bytes: &[u8]) -> Option<&'static str> {
    KNOWN_GAPS
        .iter()
        .find(|(prefix, _)| bytes.starts_with(prefix))
        .map(|(_, stage)| *stage)
}

#[test]
fn only_known_commands_are_unknown() {
    let mut unexpected: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen = vec![false; KNOWN_GAPS.len()];

    for stream in streams() {
        for event in decode(&stream.bytes, &[]).0 {
            let Event::Unknown { offset, bytes } = event else {
                continue;
            };
            match KNOWN_GAPS
                .iter()
                .position(|(prefix, _)| bytes.starts_with(prefix))
            {
                Some(index) => seen[index] = true,
                None => unexpected
                    .entry(hex(&bytes[..bytes.len().min(2)]))
                    .or_default()
                    .push(format!("{}@{offset}", stream.name)),
            }
        }
    }

    assert!(
        unexpected.is_empty(),
        "unknown commands that are not in KNOWN_GAPS:\n{}",
        unexpected
            .iter()
            .map(|(prefix, places)| format!(
                "  {prefix}: {} times, first at {}",
                places.len(),
                places[0]
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let stale: Vec<String> = KNOWN_GAPS
        .iter()
        .zip(&seen)
        .filter(|(_, seen)| !**seen)
        .map(|((prefix, stage), _)| format!("{} ({stage})", hex(prefix)))
        .collect();
    assert!(
        stale.is_empty(),
        "these KNOWN_GAPS entries are no longer unknown, remove them: {stale:?}"
    );
}

#[test]
fn no_byte_is_left_waiting() {
    for stream in streams() {
        let (_, pending) = decode(&stream.bytes, &[]);
        assert_eq!(pending, 0, "{}: bytes left waiting", stream.name);
    }
}

#[test]
fn the_events_do_not_depend_on_where_the_stream_was_cut() {
    for stream in streams() {
        let whole = decode(&stream.bytes, &[]).0;
        let length = stream.bytes.len();

        let one_byte_at_a_time: Vec<usize> = (1..length).collect();
        assert_eq!(
            decode(&stream.bytes, &one_byte_at_a_time).0,
            whole,
            "{}: one byte at a time",
            stream.name
        );

        let positions: Vec<usize> = if length <= 3_000 {
            (1..length).collect()
        } else {
            (1..400)
                .chain((400..length - 400).step_by(499))
                .chain(length - 400..length)
                .collect()
        };
        for position in positions {
            assert_eq!(
                decode(&stream.bytes, &[position]).0,
                whole,
                "{}: cut at byte {position}",
                stream.name
            );
        }
    }
}

#[test]
fn every_stream_prints_something() {
    for stream in streams() {
        let printed = decode(&stream.bytes, &[]).0.iter().any(|event| {
            matches!(
                event,
                Event::Text { .. }
                    | Event::Image(_)
                    | Event::Barcode(_)
                    | Event::Qr(_)
                    | Event::Pdf417(_)
            )
        });
        // Two of the escpresso streams are only a few settings and the cash drawer.
        let allowed_empty = stream.bytes.is_empty();
        assert!(printed || allowed_empty, "{}: prints nothing", stream.name);
    }
}

#[test]
fn known_gaps_name_the_stage_that_closes_them() {
    for (prefix, stage) in KNOWN_GAPS {
        assert!(
            known_gap(prefix).is_some() && !stage.is_empty(),
            "{}",
            hex(prefix)
        );
    }
}
