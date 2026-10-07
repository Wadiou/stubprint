//! The commands that start with `FS`.

use super::super::machine::Machine;
use super::super::reader::{Frame, Input, Step};
use super::EscPos;
use crate::event::Event;

impl EscPos {
    pub(super) fn fs(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(code) = input.get(1) else {
            return Step::Incomplete;
        };
        match code {
            0x26 | 0x2e => machine.set_kanji_mode(code == 0x26, events),
            0x43 => machine.select_kanji_system(input, events),
            0x32 => machine.define_user_kanji(input, events),
            // FS ! n, FS - n and FS W n set how two byte characters look, and FS S n1 n2 their
            // spacing. Nothing here draws those differently.
            0x21 | 0x2d | 0x57 => skip(input, 3),
            0x53 => skip(input, 4),
            0x70 => machine.parse_nv_print(input, events),
            0x71 => machine.parse_nv_define(input, events),
            // FS ( fn pL pH data... has a length.
            0x28 => self.parse_fs_function(machine, input, events),
            _ => input.unknown_run(events),
        }
    }

    /// `FS ( C pL pH fn m` with an fn of 48 chooses how text is encoded: with an m of 1 or 2 it is
    /// UTF-8. `FS ( A pL pH fn m` with an fn of 48 chooses the Kanji font. The other functions only
    /// set how two byte characters look.
    fn parse_fs_function(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(function) = input.get(2) else {
            return Step::Incomplete;
        };
        let length = match input.frame(3, 2) {
            Frame::Need | Frame::Waiting => return Step::Incomplete,
            Frame::TooBig => return input.unknown_run(events),
            Frame::Whole(length) => length,
        };
        let body = &input.bytes[5..length];
        let known = match (function, body) {
            (b'C', [48, mode, ..]) => {
                machine.flush_line(events);
                machine.state.utf8 = matches!(mode, 1 | 2 | 49 | 50);
                true
            }
            (b'A', [48, font]) => machine.select_kanji_font(*font, events),
            (b'C', _) | (b'E' | b'L', _) => true,
            _ => false,
        };
        if known {
            Step::Consumed(length)
        } else {
            input.fixed_unknown(length, events)
        }
    }
}

fn skip(input: &Input, length: usize) -> Step {
    if input.bytes.len() < length {
        return Step::Incomplete;
    }
    Step::Consumed(length)
}
