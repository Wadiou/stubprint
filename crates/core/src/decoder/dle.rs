//! The requests that start with `DLE`, which a printer answers while it prints.

use super::{Decoder, Input, Step};
use crate::event::{Event, Pulse};
use crate::status::Fault;

impl Decoder {
    /// `DLE EOT n` asks for a status, `DLE ENQ n` asks to recover from an error, and
    /// `DLE DC4 1 m t` kicks the cash drawer.
    pub(super) fn parse_dle(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(kind) = input.get(1) else {
            return Step::Incomplete;
        };
        match kind {
            0x04 | 0x05 => {
                let Some(n) = input.get(2) else {
                    return Step::Incomplete;
                };
                if kind == 0x04 {
                    let Some(answer) = self.status.real_time(n) else {
                        return input.fixed_unknown(3, events);
                    };
                    self.reply(&[answer]);
                } else if matches!(n, 1 | 2) {
                    // The program asks the printer to go on after an error that can be recovered from.
                    if self.status.fault == Some(Fault::Recoverable) {
                        let mut status = self.status;
                        status.fault = None;
                        self.set_status(status);
                    }
                } else {
                    return input.fixed_unknown(3, events);
                }
                Step::Consumed(3)
            }
            0x14 => {
                let Some(function) = input.get(2) else {
                    return Step::Incomplete;
                };
                let length = match function {
                    1 | 2 => 5,
                    _ => return input.unknown_run(events),
                };
                if input.bytes.len() < length {
                    return Step::Incomplete;
                }
                if function == 1 {
                    let (pin, time) = (input.bytes[3], input.bytes[4]);
                    if matches!(pin, 0 | 1 | 48 | 49) && (1..=8).contains(&time) {
                        self.flush_line(events);
                        events.push(Event::Pulse(Pulse {
                            pin: pin & 1,
                            on_ms: u16::from(time) * 100,
                            off_ms: u16::from(time) * 100,
                        }));
                    } else {
                        return input.fixed_unknown(length, events);
                    }
                }
                Step::Consumed(length)
            }
            _ => input.unknown_run(events),
        }
    }
}
