//! The commands that start with `FS`.

use super::{Decoder, Frame, Input, Step};
use crate::event::Event;

impl Decoder {
    pub(super) fn parse_fs(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(code) = input.get(1) else {
            return Step::Incomplete;
        };
        match code {
            // FS & turns two byte characters on and FS . turns them off.
            0x26 | 0x2e => {
                self.flush_line(events);
                self.state.kanji = code == 0x26;
                Step::Consumed(2)
            }
            // FS ! n, FS - n, FS C n and FS W n set the style and the code system of two byte
            // characters, and FS S n1 n2 their spacing. Nothing here draws those differently.
            0x21 | 0x2d | 0x43 | 0x57 => skip(input, 3),
            0x53 => skip(input, 4),
            0x70 => self.parse_nv_print(input, events),
            0x71 => self.parse_nv_define(input, events),
            // FS ( fn pL pH data... has a length.
            0x28 => self.parse_fs_function(input, events),
            _ => input.unknown_run(events),
        }
    }

    /// `FS ( C pL pH fn m` with an fn of 48 chooses how text is encoded: with an m of 1 or 2 it is
    /// UTF-8. The other functions only set how two byte characters look.
    fn parse_fs_function(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(function) = input.get(2) else {
            return Step::Incomplete;
        };
        match function {
            b'C' => match input.frame(3, 2) {
                Frame::Need | Frame::Waiting => Step::Incomplete,
                Frame::TooBig => input.unknown_run(events),
                Frame::Whole(length) => {
                    if let [48, mode, ..] = input.bytes[5..length] {
                        self.flush_line(events);
                        self.state.utf8 = matches!(mode, 1 | 2 | 49 | 50);
                    }
                    Step::Consumed(length)
                }
            },
            b'A' | b'E' | b'L' => input.framed_ignored(3, 2, events),
            _ => input.framed_unknown(3, 2, events),
        }
    }
}

fn skip(input: &Input, length: usize) -> Step {
    if input.bytes.len() < length {
        return Step::Incomplete;
    }
    Step::Consumed(length)
}
