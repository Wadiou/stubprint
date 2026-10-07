//! Two byte characters: the Kanji of a Japanese model, and the characters of the Chinese and
//! Korean ones.

use std::collections::BTreeSet;

use super::machine::Machine;
use super::reader::{Input, Step};
use crate::codepage::TwoByte;
use crate::event::{Event, UserKanji};
use crate::profile::KanjiEncoding;

/// The code system a Japanese model reads Kanji in, chosen by `FS C`. The Shift JIS 2004 system is
/// read as Shift JIS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum System {
    #[default]
    Jis,
    ShiftJis,
}

/// The Kanji font, chosen by `FS ( A`. It decides how big a character drawn with `FS 2` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Font {
    #[default]
    A,
    B,
    C,
}

impl Font {
    /// How many dots wide and tall a character is: 24 by 24, 20 by 24, or 16 by 16.
    fn size(self) -> (u8, u8) {
        match self {
            Font::A => (24, 24),
            Font::B => (20, 24),
            Font::C => (16, 16),
        }
    }

    /// How many bytes of columns draw a character: 72, 60 or 32.
    fn data_bytes(self) -> usize {
        let (width, height) = self.size();
        usize::from(width) * usize::from(height) / 8
    }
}

/// How the printer reads two byte characters now.
#[derive(Debug, Clone, Default)]
pub(super) struct Kanji {
    /// `FS &` turned them on and `FS .` has not turned them off.
    on: bool,
    system: System,
    font: Font,
    /// The codes, as sent, of the characters that `FS 2` drew.
    drawn: BTreeSet<[u8; 2]>,
}

impl Kanji {
    /// Two byte characters are being read.
    pub(super) fn is_on(&self) -> bool {
        self.on
    }

    /// Two byte characters are read in JIS code, or are no longer read.
    pub fn jis_mode(&mut self, on: bool) {
        self.system = System::Jis;
        self.on = on;
    }

    /// Two byte characters are read in Shift JIS, or are no longer read.
    pub fn shift_jis_mode(&mut self, on: bool) {
        self.system = System::ShiftJis;
        self.on = on;
    }

    /// How text is read, when the printer reads two byte characters. `model` is how the profile of
    /// the printer says its two byte characters are encoded, and only a Japanese one, which has
    /// Shift JIS there, has the choice of `FS C`: it reads JIS code until told otherwise, and with
    /// Shift JIS the lead byte of a character is enough to start one, without `FS &`.
    pub fn reading(&self, model: KanjiEncoding) -> Option<TwoByte<'_>> {
        let japanese = model == KanjiEncoding::ShiftJis;
        let encoding = if japanese && self.system == System::Jis {
            KanjiEncoding::Jis
        } else {
            model
        };
        let lead_byte_is_enough = japanese && self.system == System::ShiftJis;
        (self.on || lead_byte_is_enough).then_some(TwoByte {
            encoding,
            drawn: &self.drawn,
        })
    }
}

impl Machine {
    /// `FS &` turns two byte characters on and `FS .` turns them off.
    pub(super) fn set_kanji_mode(&mut self, on: bool, events: &mut Vec<Event>) -> Step {
        self.flush_line(events);
        self.state.kanji.on = on;
        Step::Consumed(2)
    }

    /// `FS C n` selects the code system: 0 is JIS, 1 is Shift JIS and 2 is Shift JIS 2004. The
    /// digits work too.
    pub(super) fn select_kanji_system(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(n) = input.get(2) else {
            return Step::Incomplete;
        };
        let system = match n {
            0 | 48 => System::Jis,
            1 | 2 | 49 | 50 => System::ShiftJis,
            _ => return input.fixed_unknown(3, events),
        };
        self.flush_line(events);
        self.state.kanji.system = system;
        Step::Consumed(3)
    }

    /// `FS ( A pL pH 48 m` selects the font: 0 is A, 1 is B and 2 is C. The digits work too.
    /// Returns whether the command was valid.
    pub(super) fn select_kanji_font(&mut self, m: u8, events: &mut Vec<Event>) -> bool {
        let font = match m {
            0 | 48 => Font::A,
            1 | 49 => Font::B,
            2 | 50 => Font::C,
            _ => return false,
        };
        self.flush_line(events);
        self.state.kanji.font = font;
        true
    }

    /// `FS 2 c1 c2 d1...dk` draws a character that the code c1 c2 then prints. It has no length
    /// of its own: k is 72, 60 or 32 by the font.
    pub(super) fn define_user_kanji(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let (Some(first), Some(second)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        let font = self.state.kanji.font;
        let length = 4 + font.data_bytes();
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        let (width, height) = font.size();
        if !self.store_user_kanji(
            [first, second],
            (width, height),
            &input.bytes[4..length],
            events,
        ) {
            return input.fixed_unknown(length, events);
        }
        Step::Consumed(length)
    }

    /// Remembers a character a program drew, and reports it. Returns false for a code that a
    /// printer does not let a program draw.
    pub(super) fn store_user_kanji(
        &mut self,
        code: [u8; 2],
        (width, height): (u8, u8),
        data: &[u8],
        events: &mut Vec<Event>,
    ) -> bool {
        if UserKanji::glyph(code).is_none() {
            return false;
        }
        self.flush_line(events);
        events.push(Event::DefineKanji(UserKanji {
            code,
            width,
            height,
            data: data.to_vec(),
        }));
        self.state.kanji.drawn.insert(code);
        true
    }
}
