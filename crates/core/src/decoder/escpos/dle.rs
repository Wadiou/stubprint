//! The requests that start with `DLE`, which a printer answers while it prints.

use super::super::machine::Machine;
use super::super::reader::{Input, Step, DLE};
use super::EscPos;
use crate::event::{Event, Pulse};
use crate::status::Fault;

impl EscPos {
    /// The program asks the printer to go on after an error that can be recovered from.
    fn recover(&mut self, machine: &mut Machine) {
        if machine.status.fault == Some(Fault::Recoverable) {
            let mut status = machine.status;
            status.fault = None;
            self.set_status(machine, status);
        }
    }

    /// A printer reads `DLE EOT n` and `DLE ENQ n` as soon as they come, even in the middle of what
    /// it is holding, so they are answered and taken out of the bytes that wait.
    pub(super) fn answer_held(&mut self, machine: &mut Machine, held: &mut Vec<u8>, from: usize) {
        let mut at = from;
        while at + 2 < held.len() {
            let (kind, n) = (held[at + 1], held[at + 2]);
            let answered = held[at] == DLE
                && match kind {
                    0x04 => machine
                        .status
                        .real_time(n)
                        .map(|answer| machine.reply(&[answer]))
                        .is_some(),
                    0x05 if matches!(n, 1 | 2) => {
                        self.recover(machine);
                        true
                    }
                    _ => false,
                };
            if answered {
                held.drain(at..at + 3);
            } else {
                at += 1;
            }
        }
    }

    /// `DLE EOT n` asks for a status, `DLE ENQ n` asks to recover from an error, and
    /// `DLE DC4 1 m t` kicks the cash drawer.
    pub(super) fn dle(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(kind) = input.get(1) else {
            return Step::Incomplete;
        };
        match kind {
            0x04 | 0x05 => {
                let Some(n) = input.get(2) else {
                    return Step::Incomplete;
                };
                if kind == 0x04 {
                    let Some(answer) = machine.status.real_time(n) else {
                        return input.fixed_unknown(3, events);
                    };
                    machine.reply(&[answer]);
                } else if matches!(n, 1 | 2) {
                    self.recover(machine);
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
                        machine.flush_line(events);
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
