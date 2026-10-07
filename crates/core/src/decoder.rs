//! Reads the bytes a printer receives and says what they ask for.
//!
//! The commands are read by the modules below, one for each family of commands: `ESC`, `GS`, `FS`,
//! the real time `DLE` requests, pictures, and barcodes and two dimensional codes.

mod dle;
mod esc;
mod fs;
mod graphics;
mod gs;
mod symbols;

use std::collections::{BTreeMap, BTreeSet};

use crate::bitmap::Bitmap;
use crate::codepage::{self, TextContext};
use crate::event::{Align, Event, Page, Style, TextPosition};
use crate::profile::Profile;
use crate::status::PrinterStatus;

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const FS: u8 = 0x1c;
const LF: u8 = 0x0a;
const CR: u8 = 0x0d;
const DLE: u8 = 0x10;
const HT: u8 = 0x09;
const FF: u8 = 0x0c;
const CAN: u8 = 0x18;
const BEL: u8 = 0x07;
const FIRST_TEXT_BYTE: u8 = 0x20;

/// The most bytes one command may take, so a header that promises gigabytes cannot make the decoder
/// wait for them and grow without end. A command that asks for more is reported as unknown.
const MAX_COMMAND_BYTES: usize = 8 * 1024 * 1024;
/// How far the data of a barcode may run before the byte that ends it.
const MAX_BARCODE_BYTES: usize = 4096;
/// Text with no line feed is reported in pieces of this many bytes, as a printer would wrap it.
const MAX_LINE_BYTES: usize = 4096;
/// How many bytes of pictures a printer remembers, so a stream cannot fill the memory with them.
const MAX_STORED_BYTES: usize = 32 * 1024 * 1024;

/// Not enough bytes yet to know what the command is, or to read all of it.
enum Step {
    Consumed(usize),
    Incomplete,
}

/// How much of a command with a length is there.
enum Frame {
    /// The bytes that say the length have not all arrived.
    Need,
    /// The command says it is longer than a command may be.
    TooBig,
    /// The whole command, this many bytes long, has arrived.
    Whole(usize),
    /// The length is known, and the command is not all here yet.
    Waiting,
}

/// The bytes being read, where they started in the stream, and whether the stream has ended.
struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
    at_end: bool,
}

impl Input<'_> {
    fn get(&self, index: usize) -> Option<u8> {
        self.bytes.get(index).copied()
    }

    /// An unknown command is reported once, with the control bytes that follow it. A real printer
    /// ignores those too. The run is kept open until a byte that ends it arrives, so the result
    /// does not depend on where the stream was cut.
    fn unknown_run(&self, events: &mut Vec<Event>) -> Step {
        let bytes = self.bytes;
        let mut end = if matches!(bytes[0], ESC | GS | FS) {
            2
        } else {
            1
        };
        if bytes.len() < end {
            return Step::Incomplete;
        }
        while end < bytes.len() && is_ignorable(bytes[end]) {
            end += 1;
        }
        if end == bytes.len() && !self.at_end {
            return Step::Incomplete;
        }
        events.push(Event::Unknown {
            offset: self.offset,
            bytes: bytes[..end].to_vec(),
        });
        Step::Consumed(end)
    }

    /// A command of a known length that is not read, reported as one unknown command.
    fn fixed_unknown(&self, length: usize, events: &mut Vec<Event>) -> Step {
        if self.bytes.len() < length {
            return Step::Incomplete;
        }
        events.push(Event::Unknown {
            offset: self.offset,
            bytes: self.bytes[..length].to_vec(),
        });
        Step::Consumed(length)
    }

    /// How long a command is that has its length in the bytes after its name, such as
    /// `GS ( L pL pH data`. `length_at` is where the length starts and `width` how many bytes it has.
    fn frame(&self, length_at: usize, width: usize) -> Frame {
        if self.bytes.len() < length_at + width {
            return Frame::Need;
        }
        let data = self.bytes[length_at..length_at + width]
            .iter()
            .rev()
            .fold(0usize, |total, byte| total * 256 + usize::from(*byte));
        let length = length_at + width + data;
        if length > MAX_COMMAND_BYTES {
            Frame::TooBig
        } else if self.bytes.len() < length {
            Frame::Waiting
        } else {
            Frame::Whole(length)
        }
    }

    /// A command with a length that is not read, reported as one unknown command, so its data is
    /// not taken for text or other commands.
    fn framed_unknown(&self, length_at: usize, width: usize, events: &mut Vec<Event>) -> Step {
        match self.frame(length_at, width) {
            Frame::Need | Frame::Waiting => Step::Incomplete,
            Frame::TooBig => self.unknown_run(events),
            Frame::Whole(length) => {
                events.push(Event::Unknown {
                    offset: self.offset,
                    bytes: self.bytes[..length].to_vec(),
                });
                Step::Consumed(length)
            }
        }
    }

    /// A command with a length that has nothing to show, such as a setting of the printer's
    /// hardware, read and left alone.
    fn framed_ignored(&self, length_at: usize, width: usize, events: &mut Vec<Event>) -> Step {
        match self.frame(length_at, width) {
            Frame::Need | Frame::Waiting => Step::Incomplete,
            Frame::TooBig => self.unknown_run(events),
            Frame::Whole(length) => Step::Consumed(length),
        }
    }
}

fn is_ignorable(byte: u8) -> bool {
    byte < FIRST_TEXT_BYTE && !matches!(byte, LF | CR | ESC | FS | GS)
}

/// What the printer was last told to do, which decides how the next bytes are read.
#[derive(Debug, Clone)]
struct State {
    style: Style,
    align: Align,
    codepage: u8,
    international: u8,
    barcode_height: u8,
    barcode_width: u8,
    barcode_text: TextPosition,
    barcode_font: u8,
    /// The data and the settings of two dimensional codes, by symbol and then by function.
    symbol_data: BTreeMap<u8, Vec<u8>>,
    symbol_settings: BTreeMap<u8, BTreeMap<u8, Vec<u8>>>,
    /// Whether the characters the program drew are used, and which codes it drew.
    user_characters: bool,
    user_defined: BTreeSet<u8>,
    /// Whether two byte characters are on, and whether the text is UTF-8.
    kanji: bool,
    utf8: bool,
    page_mode: bool,
    /// The picture defined with `GS *`, and the one stored with `GS ( L` waiting to be printed.
    downloaded: Option<Bitmap>,
    graphics_buffer: Option<Bitmap>,
    /// What `GS a` was asked to report on its own.
    automatic_status: u8,
}

impl Default for State {
    fn default() -> Self {
        Self {
            style: Style::default(),
            align: Align::Left,
            codepage: 0,
            international: 0,
            barcode_height: 162,
            barcode_width: 3,
            barcode_text: TextPosition::None,
            barcode_font: 0,
            symbol_data: BTreeMap::new(),
            symbol_settings: BTreeMap::new(),
            user_characters: false,
            user_defined: BTreeSet::new(),
            kanji: false,
            utf8: false,
            page_mode: false,
            downloaded: None,
            graphics_buffer: None,
            automatic_status: 0,
        }
    }
}

/// Reads the bytes a printer would receive and says what they ask for. It accepts the bytes in any
/// number of pieces, since a network or Bluetooth link can cut a command anywhere, and holds on to
/// a half received command until the rest arrives. The same bytes give the same events however
/// they are split.
///
/// A program that talks to the printer may also ask it things, such as whether the paper has run
/// out. The answers are bytes for the connection, and wait in [`Decoder::take_replies`].
#[derive(Debug)]
pub struct Decoder {
    profile: Profile,
    pending: Vec<u8>,
    /// How many bytes were consumed before the ones in `pending`.
    consumed: usize,
    state: State,
    /// The bytes of the line being printed, read in the code page when the line ends.
    line: Vec<u8>,
    /// Whether the current line has already been reported, so a line feed does not add a blank one.
    line_reported: bool,
    /// Pictures the printer keeps when it is reset: `FS q`, and the stored graphics of `GS ( L`.
    nv_bit_images: Vec<Bitmap>,
    nv_graphics: BTreeMap<(u8, u8), Bitmap>,
    download_graphics: BTreeMap<(u8, u8), Bitmap>,
    status: PrinterStatus,
    replies: Vec<u8>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::with_profile(Profile::generic())
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A decoder for one printer model, which decides what the numbers of `ESC t` mean and what
    /// the printer says about itself.
    pub fn with_profile(profile: Profile) -> Self {
        Self {
            profile,
            pending: Vec::new(),
            consumed: 0,
            state: State::default(),
            line: Vec::new(),
            line_reported: false,
            nv_bit_images: Vec::new(),
            nv_graphics: BTreeMap::new(),
            download_graphics: BTreeMap::new(),
            status: PrinterStatus::default(),
            replies: Vec::new(),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Adds received bytes and returns the events they completed.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Event> {
        self.run(chunk, false)
    }

    /// Ends the stream. A line with no line feed is printed, and bytes that never became a whole
    /// command are reported as unknown.
    pub fn finish(&mut self) -> Vec<Event> {
        let mut events = self.run(&[], true);
        self.flush_line(&mut events);
        events
    }

    /// How many received bytes are waiting for the rest of their command.
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    /// The bytes the printer answered with, since the last call, to send back to the program.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    /// What the printer says about itself when it is asked.
    pub fn status(&self) -> &PrinterStatus {
        &self.status
    }

    /// Changes what the printer says about itself, as when the paper runs out. If `GS a` asked
    /// the printer to report such a change, the report is added to the replies.
    pub fn set_status(&mut self, status: PrinterStatus) {
        let before = std::mem::replace(&mut self.status, status);
        let mask = self.state.automatic_status;
        if mask != 0 && before.changed(&self.status, mask) {
            self.replies.extend_from_slice(&self.status.automatic());
        }
    }

    fn run(&mut self, chunk: &[u8], at_end: bool) -> Vec<Event> {
        let mut bytes = std::mem::take(&mut self.pending);
        bytes.extend_from_slice(chunk);

        let mut events = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let input = Input {
                bytes: &bytes[position..],
                offset: self.consumed + position,
                at_end,
            };
            match self.parse(&input, &mut events) {
                Step::Consumed(length) => position += length,
                Step::Incomplete => break,
            }
        }

        if at_end && position < bytes.len() {
            events.push(Event::Unknown {
                offset: self.consumed + position,
                bytes: bytes[position..].to_vec(),
            });
            position = bytes.len();
        }
        self.consumed += position;
        self.pending = bytes.split_off(position);
        events
    }

    fn parse(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let bytes = input.bytes;
        match bytes[0] {
            ESC => self.parse_esc(input, events),
            GS => self.parse_gs(input, events),
            FS => self.parse_fs(input, events),
            DLE => self.parse_dle(input, events),
            LF => {
                self.line_feed(events);
                Step::Consumed(1)
            }
            CR => Step::Consumed(1),
            // A tab stays in the text as a tab, for whoever draws the line to place.
            HT => {
                self.add_text(&[HT], events);
                Step::Consumed(1)
            }
            BEL => {
                self.flush_line(events);
                events.push(Event::Beep {
                    times: 1,
                    duration: 0,
                });
                Step::Consumed(1)
            }
            // NUL and BS print nothing.
            0x00 | 0x08 => Step::Consumed(1),
            // FF prints what is in the buffer. In page mode it prints the page too.
            FF => {
                self.flush_line(events);
                if self.state.page_mode {
                    self.state.page_mode = false;
                    events.push(Event::Page(Page::Print));
                    events.push(Event::Page(Page::Standard));
                }
                Step::Consumed(1)
            }
            // CAN throws away what page mode laid out.
            CAN => {
                if self.state.page_mode {
                    self.flush_line(events);
                    events.push(Event::Page(Page::Cancel));
                }
                Step::Consumed(1)
            }
            byte if byte >= FIRST_TEXT_BYTE => {
                let length = bytes
                    .iter()
                    .position(|byte| *byte < FIRST_TEXT_BYTE)
                    .unwrap_or(bytes.len());
                self.add_text(&bytes[..length], events);
                Step::Consumed(length)
            }
            _ => input.unknown_run(events),
        }
    }

    /// Adds printable bytes to the line, and reports the line in pieces if it never ends.
    fn add_text(&mut self, text: &[u8], events: &mut Vec<Event>) {
        self.line.extend_from_slice(text);
        while self.line.len() >= MAX_LINE_BYTES {
            let rest = self.line.split_off(MAX_LINE_BYTES);
            self.flush_line(events);
            self.line = rest;
        }
    }

    /// Reports the text printed so far in the old style, then applies the change.
    fn change_style(&mut self, events: &mut Vec<Event>, change: impl FnOnce(&mut Style)) {
        self.flush_line(events);
        change(&mut self.state.style);
    }

    fn flush_line(&mut self, events: &mut Vec<Event>) {
        if self.line.is_empty() {
            return;
        }
        let context = TextContext {
            page: self.profile.code_page(self.state.codepage),
            international: self.state.international,
            user_characters: self
                .state
                .user_characters
                .then_some(&self.state.user_defined),
            kanji: self.state.kanji.then_some(self.profile.kanji),
            utf8: self.state.utf8,
        };
        let text = codepage::decode_line(&context, &self.line);
        events.push(Event::Text {
            text,
            style: self.state.style.clone(),
            align: self.state.align,
        });
        self.line.clear();
        self.line_reported = true;
    }

    fn line_feed(&mut self, events: &mut Vec<Event>) {
        self.flush_line(events);
        if !self.line_reported {
            events.push(Event::Text {
                text: String::new(),
                style: self.state.style.clone(),
                align: self.state.align,
            });
        }
        self.line_reported = false;
    }

    /// `ESC @` resets the printer. The pictures it keeps in memory, and what it says about itself,
    /// are not reset.
    fn initialize(&mut self, events: &mut Vec<Event>) {
        self.flush_line(events);
        self.state = State::default();
        events.push(Event::Initialize);
    }

    fn reply(&mut self, bytes: &[u8]) {
        self.replies.extend_from_slice(bytes);
    }

    /// How many bytes of pictures are kept in memory.
    fn stored_bytes(&self) -> usize {
        self.nv_bit_images
            .iter()
            .chain(self.nv_graphics.values())
            .chain(self.download_graphics.values())
            .chain(self.state.downloaded.iter())
            .chain(self.state.graphics_buffer.iter())
            .map(|bitmap| bitmap.data().len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;

    fn decode(bytes: &[u8], piece: usize) -> (Vec<Event>, usize) {
        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        for chunk in bytes.chunks(piece) {
            events.extend(decoder.push(chunk));
        }
        events.extend(decoder.finish());
        (events, decoder.pending_bytes())
    }

    #[test]
    fn a_character_size_is_never_more_than_8_times() {
        for n in 0..=255u8 {
            let (events, _) = decode(&[GS, b'!', n, b'x', LF], 100);
            for event in events {
                if let Event::Text { style, .. } = event {
                    assert!((1..=8).contains(&style.width), "GS ! {n}");
                    assert!((1..=8).contains(&style.height), "GS ! {n}");
                }
            }
        }
    }

    #[test]
    fn a_picture_that_promises_gigabytes_is_reported_and_not_waited_for() {
        let mut bytes = vec![GS, b'v', b'0', 0, 0xff, 0xff, 0xff, 0xff];
        bytes.extend_from_slice(b"after\n");
        // Before the stream ends, nothing is left waiting for the promised bytes.
        let mut decoder = Decoder::new();
        let events = decoder.push(&bytes);
        assert_eq!(decoder.pending_bytes(), 0);
        assert!(matches!(&events[0], Event::Unknown { offset: 0, .. }));
    }

    #[test]
    fn a_barcode_that_never_ends_is_reported_and_not_waited_for() {
        let mut bytes = vec![GS, b'k', 4];
        bytes.extend(std::iter::repeat_n(b'A', MAX_BARCODE_BYTES + 100));
        let mut decoder = Decoder::new();
        let events = decoder.push(&bytes);
        assert_eq!(decoder.pending_bytes(), 0);
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Unknown { offset: 0, .. })));
    }

    #[test]
    fn a_line_with_no_line_feed_is_reported_in_pieces_wherever_it_was_cut() {
        let bytes = vec![b'A'; MAX_LINE_BYTES * 2 + 1808];
        let (whole, _) = decode(&bytes, bytes.len());
        let (single, _) = decode(&bytes, 1);
        let (odd, _) = decode(&bytes, 777);
        assert_eq!(whole, single);
        assert_eq!(whole, odd);
        let lengths: Vec<usize> = whole
            .iter()
            .filter_map(|event| match event {
                Event::Text { text, .. } => Some(text.len()),
                _ => None,
            })
            .collect();
        assert_eq!(lengths, vec![MAX_LINE_BYTES, MAX_LINE_BYTES, 1808]);
    }

    #[test]
    fn the_number_of_a_code_page_means_what_the_printer_profile_says() {
        let bytes = [ESC, b't', 2, 0x80, LF];
        let text = |profile: &str| -> String {
            let mut decoder = Decoder::with_profile(Profile::named(profile).unwrap());
            let mut events = decoder.push(&bytes);
            events.extend(decoder.finish());
            events
                .into_iter()
                .find_map(|event| match event {
                    Event::Text { text, .. } if !text.is_empty() => Some(text),
                    _ => None,
                })
                .unwrap()
        };
        // Number 2 is CP850 on an Epson and CP932 on a Star.
        assert_eq!(text("TM-T88IV"), "Ç");
        assert_eq!(text("SP2000"), "\u{fffd}");
    }
}
