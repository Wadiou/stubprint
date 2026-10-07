//! Page mode: the commands that lay out a page in a box and print it at once.

use super::super::machine::Machine;
use super::super::reader::{Input, Step};
use super::Star;
use crate::event::{Event, Page, Position};

impl Star {
    /// `ESC GS P` and a digit, 0 to 8: select page mode (0), cancel it (1), the direction (2), the
    /// print region (3), the position down the page, absolute (4) and relative (5), print (6),
    /// print and recover (7), and throw away what was laid out (8).
    pub(super) fn page_mode(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(command) = input.get(3) else {
            return Step::Incomplete;
        };
        match command {
            b'0' => {
                machine.flush_line(events);
                if !machine.state.page_mode {
                    machine.state.page_mode = true;
                    events.push(Event::Page(Page::Enter));
                }
                Step::Consumed(4)
            }
            b'1' => {
                machine.flush_line(events);
                self.standard_mode(machine, events);
                Step::Consumed(4)
            }
            b'2' => self.page_direction(machine, input, events),
            b'3' => self.page_region(machine, input, events),
            b'4' | b'5' => self.page_position(machine, input, events, command == b'4'),
            b'6' | b'7' | b'8' if !machine.state.page_mode => Step::Consumed(4),
            b'6' => {
                machine.flush_line(events);
                events.push(Event::Page(Page::Print));
                Step::Consumed(4)
            }
            b'7' => {
                machine.flush_line(events);
                events.push(Event::Page(Page::Print));
                self.standard_mode(machine, events);
                Step::Consumed(4)
            }
            b'8' => {
                machine.flush_line(events);
                events.push(Event::Page(Page::Cancel));
                Step::Consumed(4)
            }
            _ => input.fixed_unknown(4, events),
        }
    }

    fn standard_mode(&mut self, machine: &mut Machine, events: &mut Vec<Event>) {
        if machine.state.page_mode {
            machine.state.page_mode = false;
            events.push(Event::Page(Page::Standard));
        }
    }

    fn page_direction(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(n) = input.get(4) else {
            return Step::Incomplete;
        };
        if !matches!(n, 0..=3 | b'0'..=b'3') {
            return input.fixed_unknown(5, events);
        }
        machine.flush_line(events);
        events.push(Event::Page(Page::Direction(n & 3)));
        Step::Consumed(5)
    }

    /// The region starts at x and y and is dx wide and dy high, all in dots, in two bytes each.
    fn page_region(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        if input.bytes.len() < 12 {
            return Step::Incomplete;
        }
        let word = |at: usize| u16::from(input.bytes[at]) + u16::from(input.bytes[at + 1]) * 256;
        let (width, height) = (word(8), word(10));
        if width == 0 || height == 0 {
            return input.fixed_unknown(12, events);
        }
        machine.flush_line(events);
        events.push(Event::Page(Page::Area {
            x: word(4),
            y: word(6),
            width,
            height,
        }));
        Step::Consumed(12)
    }

    fn page_position(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
        absolute: bool,
    ) -> Step {
        let (Some(low), Some(high)) = (input.get(4), input.get(5)) else {
            return Step::Incomplete;
        };
        if !machine.state.page_mode {
            return Step::Consumed(6);
        }
        let value = u16::from(low) + u16::from(high) * 256;
        machine.flush_line(events);
        events.push(Event::Page(Page::Vertical(if absolute {
            Position::Absolute(value)
        } else {
            Position::Relative(value as i16)
        })));
        Step::Consumed(6)
    }
}
