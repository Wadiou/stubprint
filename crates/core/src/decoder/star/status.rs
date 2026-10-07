//! What the printer says: the status it sends by itself, and the answers to requests.

use super::super::machine::Machine;
use super::super::reader::{Input, Step};
use super::super::reader::{CAN, ESC, GS, LF};
use super::Star;
use crate::event::Event;

/// `ESC GS ETX`, which asks for the print end counter.
const ETX: u8 = 0x03;

impl Star {
    /// Sends the nine bytes of the automatic status, and with them tells about an `ETB`.
    pub(super) fn send_status(&mut self, machine: &mut Machine) {
        let status = machine
            .status
            .star_asb(self.kept.etb_pending(), self.kept.etb_counter());
        self.kept.etb_told();
        machine.reply(&status);
    }

    /// `ESC ACK SOH` asks for the status, whatever the settings say, and `ESC ACK CAN` resets the
    /// printer at once.
    pub(super) fn ack(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        match input.get(2) {
            None => Step::Incomplete,
            Some(0x01) => {
                self.send_status(machine);
                Step::Consumed(3)
            }
            Some(CAN) => {
                machine.line.clear();
                machine.line_reported = false;
                self.initialize(machine, events);
                self.kept.clear_counters();
                Step::Consumed(3)
            }
            Some(_) => input.unknown_run(events),
        }
    }

    /// `ETB` counts a point in the print job that the program can ask the status about.
    pub(super) fn etb(&mut self, machine: &mut Machine, events: &mut Vec<Event>) -> Step {
        machine.flush_line(events);
        self.kept.count_etb();
        if self.kept.reports_by_itself() {
            self.send_status(machine);
        }
        Step::Consumed(1)
    }

    /// `ESC RS a n` picks when the status is sent: 0 never, 1 whenever it changes (and 2 and 3
    /// also when a connection is made). 16 sets it back and 255 sends it now.
    pub(super) fn status_reporting(
        &mut self,
        machine: &mut Machine,
        n: u8,
        _events: &mut Vec<Event>,
    ) -> bool {
        match n {
            0..=3 | b'0'..=b'3' => self.kept.set_reporting(n & 0x03),
            16 => self.kept.set_reporting(0),
            255 => self.send_status(machine),
            _ => return false,
        }
        true
    }

    /// `ESC RS E n` clears the counter of `ETB` commands.
    pub(super) fn clear_etb(
        &mut self,
        _machine: &mut Machine,
        n: u8,
        _events: &mut Vec<Event>,
    ) -> bool {
        if !matches!(n, 0 | b'0') {
            return false;
        }
        self.kept.clear_counters();
        true
    }

    /// `ESC GS ETX s n1 n2` works with the counter of print jobs: s of 0 asks for it, 1 counts a job
    /// when it is printed and asks, 2 clears it, 3 and 4 start and end a document, and 5 is the
    /// time to wait for data, that n1 of 1 sets and 2 asks for. What is sent back is what was sent,
    /// followed by the answer and a NUL.
    pub(super) fn print_end(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        events: &mut Vec<Event>,
    ) -> Step {
        let (Some(s), Some(n1), Some(n2)) = (input.get(3), input.get(4), input.get(5)) else {
            return Step::Incomplete;
        };
        let answer = match s {
            0 => Some(self.kept.print_end()),
            1 => {
                machine.flush_line(events);
                self.kept.count_print_end();
                Some(self.kept.print_end())
            }
            2 => {
                self.kept.clear_print_end();
                None
            }
            3 | 4 => {
                machine.flush_line(events);
                None
            }
            5 => match n1 {
                0 => {
                    self.kept.set_timeout(3);
                    None
                }
                1 => {
                    self.kept.set_timeout(n2);
                    None
                }
                2 => Some(self.kept.timeout()),
                _ => return input.fixed_unknown(6, events),
            },
            _ => return input.fixed_unknown(6, events),
        };
        if let Some(answer) = answer {
            machine.reply(&[ESC, GS, ETX, s, n1, n2, answer, 0]);
        }
        Step::Consumed(6)
    }

    /// `ESC # * LF NUL` asks which printer it is: `ESC # * ,` the model and its version, `LF`, `NUL`.
    /// The version is 1.0, which is all there is to say about a program.
    pub(super) fn version(&mut self, machine: &mut Machine) {
        let mut answer = vec![ESC, b'#', b'*', b','];
        answer.extend_from_slice(format!("{}Ver1.0", machine.profile.identity.model).as_bytes());
        answer.extend_from_slice(&[LF, 0]);
        machine.reply(&answer);
    }
}
