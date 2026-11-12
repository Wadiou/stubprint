//! The working parts of a printer that every command language shares: the line being printed, how
//! the next characters look, what the printer remembers, what it says about itself, and what it
//! has said. A command language is only the way bytes ask for these things.

use std::collections::{BTreeMap, BTreeSet};

use super::kanji::Kanji;
use super::memory::Memory;
use super::reader::MAX_LINE_BYTES;
use crate::bitmap::Bitmap;
use crate::codepage::{self, TextContext};
use crate::counters::Counters;
use crate::event::{Align, Event, Style};
use crate::layout::Layout;
use crate::profile::Profile;
use crate::snapshot::Snapshot;
use crate::status::PrinterStatus;

/// What a number of the command that picks a code page means. Each language numbers them its own
/// way, and in ESC/POS the printer decides.
pub(super) type CodePages = for<'a> fn(&'a Profile, u8) -> Option<&'a str>;

/// What the printer was last told to do, which decides how the next bytes are read. `ESC @` and the
/// commands like it take it all back.
#[derive(Debug, Clone)]
pub(super) struct State {
    pub(super) style: Style,
    pub(super) align: Align,
    pub(super) codepage: u8,
    pub(super) international: u8,
    /// The data and the settings of two dimensional codes, by symbol and then by function.
    pub(super) symbol_data: BTreeMap<u8, Vec<u8>>,
    pub(super) symbol_settings: BTreeMap<u8, BTreeMap<u8, Vec<u8>>>,
    /// Whether the characters the program drew are used, and which codes it drew.
    pub(super) user_characters: bool,
    pub(super) user_defined: BTreeSet<u8>,
    /// How two byte characters are read, and whether the text is UTF-8.
    pub(super) kanji: Kanji,
    pub(super) utf8: bool,
    pub(super) page_mode: bool,
    /// The picture defined with `GS *`, and the one stored with `GS ( L` waiting to be printed.
    pub(super) downloaded: Option<Bitmap>,
    pub(super) graphics_buffer: Option<Bitmap>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            style: Style::default(),
            align: Align::Left,
            codepage: 0,
            international: 0,
            symbol_data: BTreeMap::new(),
            symbol_settings: BTreeMap::new(),
            user_characters: false,
            user_defined: BTreeSet::new(),
            kanji: Kanji::default(),
            utf8: false,
            page_mode: false,
            downloaded: None,
            graphics_buffer: None,
        }
    }
}

/// The parts of a printer that its command language works on.
#[derive(Debug)]
pub(super) struct Machine {
    pub(super) profile: Profile,
    pub(super) state: State,
    /// The bytes of the line being printed, read in the code page when the line ends.
    pub(super) line: Vec<u8>,
    /// Whether the current line has already been reported, so a line feed does not add a blank one.
    pub(super) line_reported: bool,
    /// Pictures the printer keeps when it is reset: `FS q`, and the stored graphics of `GS ( L`.
    pub(super) memory: Memory,
    /// What the printer says about itself when it is asked.
    pub(super) status: PrinterStatus,
    /// What the printer counted about its own work, which a program can ask for.
    pub(super) counters: Counters,
    /// The bytes the printer answered with, which the connection has not taken yet.
    pub(super) replies: Vec<u8>,
    code_pages: CodePages,
}

impl Machine {
    pub(super) fn new(profile: Profile, code_pages: CodePages) -> Self {
        Self {
            profile,
            state: State::default(),
            line: Vec::new(),
            line_reported: false,
            memory: Memory::default(),
            status: PrinterStatus::default(),
            counters: Counters::default(),
            replies: Vec::new(),
            code_pages,
        }
    }

    /// Adds printable bytes to the line, and reports the line in pieces if it never ends.
    pub(super) fn add_text(&mut self, text: &[u8], events: &mut Vec<Event>) {
        self.line.extend_from_slice(text);
        while self.line.len() >= MAX_LINE_BYTES {
            let rest = self.line.split_off(MAX_LINE_BYTES);
            self.flush_line(events);
            self.line = rest;
        }
    }

    /// Reports the text printed so far in the old style, then applies the change.
    pub(super) fn change_style(
        &mut self,
        events: &mut Vec<Event>,
        change: impl FnOnce(&mut Style),
    ) {
        self.flush_line(events);
        change(&mut self.state.style);
    }

    /// Reports the line printed so far, read in the code page that is selected.
    pub(super) fn flush_line(&mut self, events: &mut Vec<Event>) {
        if self.line.is_empty() {
            return;
        }
        let context = TextContext {
            page: (self.code_pages)(&self.profile, self.state.codepage),
            international: self.state.international,
            user_characters: self
                .state
                .user_characters
                .then_some(&self.state.user_defined),
            kanji: self.state.kanji.reading(self.profile.kanji),
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

    /// Ends the line. A line feed after a line that was already reported adds no blank line.
    pub(super) fn line_feed(&mut self, events: &mut Vec<Event>) {
        self.flush_line(events);
        if !self.line_reported {
            events.push(Event::Text {
                text: String::new(),
                style: self.state.style.clone(),
                align: self.state.align,
            });
        }
        self.line_reported = false;
        events.push(Event::LineFeed);
    }

    /// Resets the printer. The pictures it keeps in memory, and what it says about itself, are not
    /// reset.
    pub(super) fn initialize(&mut self, events: &mut Vec<Event>) {
        self.flush_line(events);
        self.state = State::default();
        events.push(Event::Initialize);
    }

    /// Throws away the line that was being printed, as when a command cancels the print data.
    pub(super) fn discard_line(&mut self) {
        self.line.clear();
        self.line_reported = false;
    }

    /// What the printer believes now, with the layout settings the events have set.
    pub(super) fn snapshot(&self, layout: &Layout) -> Snapshot {
        Snapshot {
            style: self.state.style.clone(),
            align: self.state.align,
            code_page_number: self.state.codepage,
            code_page: (self.code_pages)(&self.profile, self.state.codepage).map(str::to_string),
            international: self.state.international,
            utf8: self.state.utf8,
            user_characters: self.state.user_characters,
            kanji: self.state.kanji.is_on(),
            page_mode: self.state.page_mode,
            layout: layout.clone(),
        }
    }

    pub(super) fn reply(&mut self, bytes: &[u8]) {
        self.replies.extend_from_slice(bytes);
    }

    /// How many bytes of pictures are kept in memory.
    pub(super) fn stored_bytes(&self) -> usize {
        self.memory.bytes()
            + [&self.state.downloaded, &self.state.graphics_buffer]
                .into_iter()
                .flatten()
                .map(|bitmap| bitmap.data().len())
                .sum::<usize>()
    }
}
