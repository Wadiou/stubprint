//! Where things go: feeds, margins, tabs, positions, cuts, and the drawer and the buzzer.

use super::super::machine::Machine;
use super::super::reader::{Input, Step};
use super::Star;
use crate::dialect::Dialect;
use crate::event::{Align, CutMode, Event, Feed, FeedAmount, LineSpacing, Position, Pulse};

/// The most tabs a Star printer remembers.
const MAX_TAB_STOPS: usize = 16;
/// The dots in a millimetre, at the 203 dots to the inch of these printers.
const DOTS_PER_MM: u16 = 8;

impl Star {
    /// The commands of layout. Gives nothing for a command that is not one of those.
    pub(super) fn layout_command(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        code: u8,
        events: &mut Vec<Event>,
    ) -> Option<Step> {
        let line = self.dialect == Dialect::StarLine;
        Some(match code {
            0x61 => self.argument(machine, input, events, 2, Self::feed_lines),
            0x7a => self.argument(machine, input, events, 2, Self::line_feed_amount),
            0x30 => self.line_spacing(machine, events, 3, 2),
            0x4a => self.argument(machine, input, events, 2, Self::feed_quarter_millimetres),
            0x49 => self.argument(machine, input, events, 2, Self::feed_eighth_millimetres),
            0x43 => self.page_length(input),
            0x42 if line => self.vertical_tabs(input),
            0x6c => self.argument(machine, input, events, 2, Self::left_margin),
            0x51 => self.argument(machine, input, events, 2, Self::right_margin),
            0x44 => self.tab_stops(machine, input, events),
            0x64 => self.argument(machine, input, events, 2, Self::cut),
            0x07 => self.arguments_of_drawer(input, events),
            0x40 => {
                self.initialize(machine, events);
                Step::Consumed(2)
            }
            _ => return None,
        })
    }

    /// How wide a character is, with the room right of it and how many times wider it is, in dots.
    /// The margins and the tabs count in characters of this width.
    fn pitch(&self, machine: &Machine) -> u16 {
        let font_width = match machine.state.style.font {
            0 => 12,
            1 => 9,
            _ => 16,
        };
        (font_width + u16::from(machine.state.style.spacing)) * u16::from(machine.state.style.width)
    }

    /// `ESC a n` feeds n lines.
    fn feed_lines(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        if !(1..=127).contains(&n) {
            return false;
        }
        machine.flush_line(events);
        events.push(Event::Feed(Feed {
            amount: FeedAmount::Lines(n),
            reverse: false,
        }));
        true
    }

    /// `ESC z n` makes a line 3 or 4 millimetres.
    fn line_feed_amount(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let millimetres = match n {
            0 | b'0' => 3,
            1 | b'1' => 4,
            _ => return false,
        };
        self.line_spacing(machine, events, millimetres, 0);
        true
    }

    /// Sets how far apart lines are. `length` is how many bytes the command was, if it ends here.
    fn line_spacing(
        &mut self,
        machine: &mut Machine,
        events: &mut Vec<Event>,
        millimetres: u16,
        length: usize,
    ) -> Step {
        machine.flush_line(events);
        events.push(Event::LineSpacing(LineSpacing::Dots(
            millimetres * DOTS_PER_MM,
        )));
        Step::Consumed(length)
    }

    /// `ESC J n` feeds n quarters of a millimetre.
    fn feed_quarter_millimetres(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        self.feed_dots(machine, u16::from(n) * DOTS_PER_MM / 4, n != 0, events)
    }

    /// `ESC I n` feeds n eighths of a millimetre.
    fn feed_eighth_millimetres(
        &mut self,
        machine: &mut Machine,
        n: u8,
        events: &mut Vec<Event>,
    ) -> bool {
        self.feed_dots(machine, u16::from(n), n != 0, events)
    }

    fn feed_dots(
        &mut self,
        machine: &mut Machine,
        dots: u16,
        valid: bool,
        events: &mut Vec<Event>,
    ) -> bool {
        if !valid {
            return false;
        }
        machine.flush_line(events);
        events.push(Event::Feed(Feed {
            amount: FeedAmount::Units(dots),
            reverse: false,
        }));
        true
    }

    /// `FF` feeds to the top of the next page, which page mode does not do.
    pub(super) fn form_feed(&mut self, machine: &mut Machine, events: &mut Vec<Event>) -> Step {
        if !machine.state.page_mode {
            machine.flush_line(events);
            events.push(Event::Feed(Feed {
                amount: FeedAmount::Page,
                reverse: false,
            }));
        }
        Step::Consumed(1)
    }

    /// `ESC C n` and `ESC C NUL n` set how long a page is, which only the form feed uses.
    fn page_length(&mut self, input: &Input) -> Step {
        let Some(first) = input.get(2) else {
            return Step::Incomplete;
        };
        if first != 0 {
            return Step::Consumed(3);
        }
        if input.get(3).is_none() {
            return Step::Incomplete;
        }
        Step::Consumed(4)
    }

    /// `ESC B n1 ... nk NUL` sets the vertical tabs, which only the vertical tab uses.
    fn vertical_tabs(&mut self, input: &Input) -> Step {
        match input.stops(MAX_TAB_STOPS) {
            Some((_, length)) => Step::Consumed(length),
            None => Step::Incomplete,
        }
    }

    /// `ESC l n` leaves a margin of n characters at the left.
    fn left_margin(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        machine.flush_line(events);
        events.push(Event::Margin(self.pitch(machine) * u16::from(n)));
        true
    }

    /// `ESC Q n` prints in the n characters from the left edge.
    fn right_margin(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        machine.flush_line(events);
        events.push(Event::PrintWidth(self.pitch(machine) * u16::from(n)));
        true
    }

    /// `ESC D n1 ... nk NUL` sets the columns that the tab character jumps to.
    fn tab_stops(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some((stops, length)) = input.stops(MAX_TAB_STOPS) else {
            return Step::Incomplete;
        };
        machine.flush_line(events);
        events.push(Event::TabStops(stops));
        Step::Consumed(length)
    }

    /// `ESC d n` cuts the paper, and for 2 and 3 feeds it to the cutter first.
    fn cut(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let mode = match n {
            0 | b'0' | 2 | b'2' => CutMode::Full,
            1 | b'1' | 3 | b'3' => CutMode::Partial,
            _ => return false,
        };
        machine.flush_line(events);
        events.push(Event::Cut(mode));
        true
    }

    /// `ESC GS A n1 n2` moves to a position counted from the left margin, and `ESC GS R n1 n2` from
    /// where the print position is. Both are 65536 around: a move to the left is written as the
    /// complement.
    pub(super) fn position(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(low), Some(high)) = (input.get(3), input.get(4)) else {
            return Step::Incomplete;
        };
        let value = u16::from(low) + u16::from(high) * 256;
        machine.flush_line(events);
        events.push(Event::Position(if input.bytes[2] == 0x41 {
            Position::Absolute(value)
        } else {
            Position::Relative(value as i16)
        }));
        Step::Consumed(5)
    }

    /// `ESC GS a n` aligns what is printed in the print region.
    pub(super) fn align(&mut self, machine: &mut Machine, n: u8, events: &mut Vec<Event>) -> bool {
        let align = match n {
            0 | b'0' => Align::Left,
            1 | b'1' => Align::Center,
            2 | b'2' => Align::Right,
            _ => return false,
        };
        machine.flush_line(events);
        machine.state.align = align;
        events.push(Event::Align(align));
        true
    }

    /// `ESC BEL n1 n2` sets how long the drawer is driven and how long it rests, in tens of
    /// milliseconds.
    fn arguments_of_drawer(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let (Some(on), Some(off)) = (input.get(2), input.get(3)) else {
            return Step::Incomplete;
        };
        if !(1..=127).contains(&on) || !(1..=127).contains(&off) {
            return input.fixed_unknown(4, events);
        }
        self.kept
            .set_drawer(0, u16::from(on) * 10, u16::from(off) * 10);
        Step::Consumed(4)
    }

    /// `BEL` and `FS` drive the first drawer, `SUB` and `EM` the second.
    pub(super) fn drawer(
        &mut self,
        machine: &mut Machine,
        index: u8,
        events: &mut Vec<Event>,
    ) -> Step {
        self.pulse(machine, index, events);
        Step::Consumed(1)
    }

    /// Drives a drawer, for as long as the printer was told to.
    fn pulse(&mut self, machine: &mut Machine, index: u8, events: &mut Vec<Event>) {
        machine.flush_line(events);
        let (on_ms, off_ms) = self.kept.drawer(index);
        events.push(Event::Pulse(Pulse {
            pin: index,
            on_ms,
            off_ms,
        }));
    }

    /// `ESC GS BEL m t1 t2` rings the buzzer, for t1 times 20 milliseconds.
    pub(super) fn buzzer(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(terminal), Some(on), Some(off)) = (input.get(3), input.get(4), input.get(5))
        else {
            return Step::Incomplete;
        };
        if !matches!(terminal, 1 | 2 | b'1' | b'2') || on == 0 || off == 0 {
            return input.fixed_unknown(6, events);
        }
        machine.flush_line(events);
        events.push(Event::Beep {
            times: 1,
            duration: beep_duration(on),
        });
        Step::Consumed(6)
    }

    /// `ESC GS EM DC1 m n1 n2` sets the pulse of an external buzzer and `ESC GS EM DC2 m n1 n2`
    /// rings it n1 times. `ESC GS EM DC3 m n1 n2` and `ESC GS EM DC4 m` do the same for the
    /// drawers, which StarPRNT has four of.
    pub(super) fn external_device(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(kind), Some(terminal)) = (input.get(3), input.get(4)) else {
            return Step::Incomplete;
        };
        let length = if kind == 0x14 { 5 } else { 7 };
        if input.bytes.len() < length {
            return Step::Incomplete;
        }
        let terminal = match terminal {
            1..=4 => terminal - 1,
            b'1'..=b'4' => terminal - b'1',
            _ => return input.fixed_unknown(length, events),
        };
        let (first, second) = (
            input.get(5).unwrap_or_default(),
            input.get(6).unwrap_or_default(),
        );
        match kind {
            // The two buzzer terminals, and the four terminals of the drawers.
            0x11 if terminal < 2 => self.kept.set_buzzer(terminal, first, second),
            0x12 if terminal < 2 => {
                machine.flush_line(events);
                events.push(Event::Beep {
                    times: first,
                    duration: beep_duration(self.kept.buzzer(terminal).0),
                });
            }
            0x13 if self.dialect == Dialect::StarPrnt => {
                let (on, off) = (u16::from(first) * 10, u16::from(second) * 10);
                self.kept.set_drawer(terminal, on, off);
            }
            0x14 if self.dialect == Dialect::StarPrnt => self.pulse(machine, terminal, events),
            _ => return input.fixed_unknown(length, events),
        }
        Step::Consumed(length)
    }
}

/// How long a buzzer sounds, in units of 100 ms, for an energizing time of t times 20 ms.
fn beep_duration(t: u8) -> u8 {
    (u16::from(t) * 2).div_ceil(10) as u8
}
