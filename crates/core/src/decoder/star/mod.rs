//! The command languages of Star printers: Star Line mode and StarPRNT. They share most of their
//! commands, so one reader handles both, and the few commands that only one has are checked with
//! the dialect. Their bytes mean other things than the same bytes in ESC/POS, so a decoder reads
//! one language only, which the port it listens on is set to.
//!
//! The commands are read by the module of their family: text, layout, pictures, codes, status and
//! page mode. Distances are in dots, at 8 to the millimetre, as the languages say them.

mod codes;
mod images;
mod kept;
mod layout;
mod page;
mod status;
mod text;

use super::language::Language;
use super::machine::{CodePages, Machine};
use super::reader::{Input, Step, BEL, CAN, CR, ESC, FF, FIRST_TEXT_BYTE, FS, HT, LF};
use crate::dialect::Dialect;
use crate::event::Event;
use crate::status::PrinterStatus;

use kept::Kept;

const ENQ: u8 = 0x05;
const EOT: u8 = 0x04;
const SO: u8 = 0x0e;
const SI: u8 = 0x0f;
const VT: u8 = 0x0b;
const DC2: u8 = 0x12;
const DC4: u8 = 0x14;
const ETB: u8 = 0x17;
const EM: u8 = 0x19;
const SUB: u8 = 0x1a;
const RS: u8 = 0x1e;

/// What the commands set, which `ESC @` and `CAN` take back.
#[derive(Debug, Clone, Default)]
struct Mode {
    /// The picture that raster mode is drawing, while raster mode is on.
    raster: Option<images::Raster>,
    qr: codes::QrSettings,
    pdf417: codes::Pdf417Settings,
}

/// A printer that is set to Star Line mode or to StarPRNT.
#[derive(Debug)]
pub(super) struct Star {
    /// Which of the two languages it reads. Most commands are the same in both.
    dialect: Dialect,
    mode: Mode,
    /// What it keeps when `ESC @` and `CAN` set the commands back.
    kept: Kept,
}

impl Star {
    pub(super) fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            mode: Mode::default(),
            kept: Kept::default(),
        }
    }

    /// `ESC @` sets the commands back, and what the printer keeps stays.
    fn initialize(&mut self, machine: &mut Machine, events: &mut Vec<Event>) {
        machine.initialize(events);
        self.mode = Mode::default();
    }

    /// `CAN` throws away what is waiting to be printed, and sets the commands back.
    fn cancel(&mut self, machine: &mut Machine, events: &mut Vec<Event>) -> Step {
        machine.discard_line();
        self.initialize(machine, events);
        self.kept.clear_counters();
        Step::Consumed(1)
    }
}

impl Language for Star {
    fn parse(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        self.read(machine, input, events)
    }

    fn code_pages(&self) -> CodePages {
        |_, number| text::code_page(number)
    }

    /// The status is sent by itself when the program asked for that, and something changed.
    fn status_changed(&mut self, machine: &mut Machine, before: &PrinterStatus) {
        if self.kept.reports_by_itself() && *before != machine.status {
            self.send_status(machine);
        }
    }
}

impl Star {
    /// Reads one command, or the text up to the next one.
    fn read(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let byte = input.bytes[0];
        // Raster mode has its own commands, and ignores the rest.
        if self.mode.raster.is_some() && byte != ESC {
            return self.raster_byte(machine, input, events);
        }
        let line_mode = self.dialect == Dialect::StarLine;
        match byte {
            ESC => self.esc(machine, input, events),
            LF => {
                machine.line_feed(events);
                Step::Consumed(1)
            }
            // A carriage return is ignored, and so are the NUL that ends commands and the backspace.
            CR | 0x00 | 0x08 => Step::Consumed(1),
            HT => {
                machine.add_text(&[HT], events);
                Step::Consumed(1)
            }
            FF => self.form_feed(machine, events),
            VT if line_mode => Step::Consumed(1),
            BEL | FS => self.drawer(machine, 0, events),
            SUB | EM => self.drawer(machine, 1, events),
            SO if line_mode => self.style(machine, events, 1, |style| style.width = 2),
            DC4 if line_mode => self.style(machine, events, 1, |style| style.width = 1),
            SI => self.style(machine, events, 1, |style| style.upside_down = true),
            DC2 => self.style(machine, events, 1, |style| style.upside_down = false),
            ETB => self.etb(machine, events),
            ENQ if line_mode => {
                machine.reply(&[machine.status.star_enq()]);
                Step::Consumed(1)
            }
            EOT if line_mode => {
                machine.reply(&[machine.status.star_eot()]);
                Step::Consumed(1)
            }
            CAN => self.cancel(machine, events),
            byte if byte >= FIRST_TEXT_BYTE => {
                let length = input
                    .bytes
                    .iter()
                    .position(|byte| *byte < FIRST_TEXT_BYTE)
                    .unwrap_or(input.bytes.len());
                machine.add_text(&input.bytes[..length], events);
                Step::Consumed(length)
            }
            _ => input.unknown_run(events),
        }
    }

    /// `ESC` and the byte after it.
    fn esc(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(code) = input.get(1) else {
            return Step::Incomplete;
        };
        match code {
            RS => self.esc_rs(machine, input, events),
            0x1d => self.esc_gs(machine, input, events),
            FS => self.esc_fs(machine, input, events),
            // ESC ACK SOH asks for the status and ESC ACK CAN resets the printer.
            0x06 => self.ack(machine, input, events),
            0x2a => self.esc_star(machine, input, events),
            // ESC FF NUL and ESC FF EOT end raster mode with what its settings say.
            FF => self.raster_end(machine, input, events),
            0x23 | 0x3f => self.esc_hash(machine, input, events),
            // ESC SYN and a number and a byte: the presenter, which draws nothing.
            0x16 => self.fixed(input, 4),
            0x62 => self.barcode(machine, input, events),
            0x4b | 0x4c | 0x6b | 0x58 => self.bit_image(machine, input, events),
            0x26 => self.download_characters(machine, input, events),
            0x72 => self.kanji_download(machine, input, events),
            0x29 => self.esc_paren(input, events),
            _ => {
                if let Some(step) = self.text_command(machine, input, code, events) {
                    return step;
                }
                if let Some(step) = self.layout_command(machine, input, code, events) {
                    return step;
                }
                input.unknown_run(events)
            }
        }
    }

    /// `ESC RS` and a letter: settings of the printer.
    fn esc_rs(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(letter) = input.get(2) else {
            return Step::Incomplete;
        };
        let line = self.dialect == Dialect::StarLine;
        match letter {
            b'F' => self.argument(machine, input, events, 3, Self::select_font),
            b'L' if line => self.argument(machine, input, events, 3, Self::logo_batch),
            b'a' => self.argument(machine, input, events, 3, Self::status_reporting),
            b'E' => self.argument(machine, input, events, 3, Self::clear_etb),
            // The density, the speed, the colours, and in StarPRNT the top margin, the print area
            // and the place of use: settings of the machine, with no mark in what is drawn.
            b'd' | b'r' | b'c' | b'C' => self.argument(machine, input, events, 3, Self::setting),
            b'T' | b'A' | b'R' if !line => self.argument(machine, input, events, 3, Self::setting),
            _ => input.unknown_run(events),
        }
    }

    /// A setting of the machine that does not change what is drawn.
    fn setting(&mut self, _machine: &mut Machine, _n: u8, _events: &mut Vec<Event>) -> bool {
        true
    }

    /// `ESC GS` and a byte: most of what has more than one byte to say.
    fn esc_gs(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(code) = input.get(2) else {
            return Step::Incomplete;
        };
        match code {
            0x74 => self.argument(machine, input, events, 3, Self::select_code_page),
            0x3d => self.blank_code_page(input, events),
            0x62 => self.argument(machine, input, events, 3, Self::smoothing),
            0x41 | 0x52 => self.position(machine, input, events),
            0x61 => self.argument(machine, input, events, 3, Self::align),
            0x07 => self.buzzer(machine, input, events),
            0x19 => self.external_device(machine, input, events),
            0x03 => self.print_end(machine, input, events),
            // ESC GS # m N n1 n2 n3 n4 LF NUL sets a memory switch.
            0x23 => self.fixed(input, 11),
            0x50 => self.page_mode(machine, input, events),
            0x78 => self.pdf417(machine, input, events),
            0x79 => self.qr(machine, input, events),
            0x28 | 0x38 => self.gs_function(machine, input, code, events),
            0x53 if self.dialect == Dialect::StarPrnt => {
                self.raster_graphics(machine, input, events, false)
            }
            0x58 if self.dialect == Dialect::StarPrnt => {
                self.raster_graphics(machine, input, events, true)
            }
            // Commands with a length: the settings of the printer, which draw nothing.
            0x29 | 0x4a => self.framed(input, if code == 0x29 { 4 } else { 3 }, events),
            // ESC GS h, c and g, which turn and shrink the page and time the start of printing.
            0x68 => self.fixed(input, 7),
            0x63 => self.fixed(input, 5),
            0x67 => self.fixed(input, 6),
            0x6d => self.fixed(input, 6),
            // The auto logo: a number, a macro of a length, and the registering.
            0x2f => self.auto_logo(input, events),
            // ESC GS SUB DC1, DC2 and DC3 m t1 t2 set the snout of a presenter.
            0x1a => self.fixed(input, 7),
            _ => input.unknown_run(events),
        }
    }

    /// `ESC GS (` and `ESC GS 8` and a letter: graphics, which are the commands of ESC/POS with
    /// an `ESC` before them, and the symbols of GS1.
    fn gs_function(
        &mut self,
        machine: &mut Machine,
        input: &Input,
        code: u8,
        events: &mut Vec<Event>,
    ) -> Step {
        let Some(letter) = input.get(3) else {
            return Step::Incomplete;
        };
        match (code, letter) {
            (0x28, b'L') => without_esc(machine, input, events, |machine, inner, events| {
                machine.parse_graphics(inner, 5, 2, events)
            }),
            (0x38, b'L') => without_esc(machine, input, events, |machine, inner, events| {
                machine.parse_graphics(inner, 7, 4, events)
            }),
            (0x28, b'k') => without_esc(machine, input, events, Machine::parse_symbol),
            _ => input.unknown_run(events),
        }
    }

    /// `ESC FS q` and `ESC FS p`, which register and print the logos as `FS q` and `FS p` do.
    fn esc_fs(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let Some(letter) = input.get(2) else {
            return Step::Incomplete;
        };
        if self.dialect != Dialect::StarLine {
            return input.unknown_run(events);
        }
        match letter {
            b'q' => without_esc(machine, input, events, Machine::parse_nv_define),
            b'p' => without_esc(machine, input, events, Machine::parse_nv_print),
            _ => input.unknown_run(events),
        }
    }

    /// `ESC # * LF NUL` asks the version of the printer, and `ESC ? LF NUL` resets it.
    fn esc_hash(&mut self, machine: &mut Machine, input: &Input, events: &mut Vec<Event>) -> Step {
        let (kind, rest) = if input.bytes[1] == b'#' {
            (b'#', &[b'*', LF, 0][..])
        } else {
            (b'?', &[LF, 0][..])
        };
        let length = 2 + rest.len();
        if input.bytes.len() < length {
            return if input.bytes[2..].iter().zip(rest).all(|(a, b)| a == b) {
                Step::Incomplete
            } else {
                input.unknown_run(events)
            };
        }
        if &input.bytes[2..length] != rest {
            return input.unknown_run(events);
        }
        if kind == b'#' {
            self.version(machine);
        } else {
            self.initialize(machine, events);
        }
        Step::Consumed(length)
    }

    /// `ESC )` and a letter and a length: the information a printer is asked for, or told, that
    /// draws nothing. `ESC SYN` and a byte or two are the commands of a presenter.
    fn esc_paren(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        self.framed(input, 3, events)
    }

    /// The auto logo: `ESC GS /` and a number, a number or a macro that has a length, or a letter.
    fn auto_logo(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        match input.get(3) {
            None => Step::Incomplete,
            Some(b'1' | b'2' | b'5' | b'6') => self.fixed(input, 5),
            Some(b'3' | b'4') => self.framed(input, 4, events),
            Some(b'W' | b'C') => self.fixed(input, 4),
            Some(_) => input.unknown_run(events),
        }
    }

    /// A command that is read and has no mark in what is drawn, of a fixed length.
    fn fixed(&mut self, input: &Input, length: usize) -> Step {
        if input.bytes.len() < length {
            Step::Incomplete
        } else {
            Step::Consumed(length)
        }
    }

    /// A command that is read and has no mark in what is drawn, and has its length in the two
    /// bytes at `length_at`.
    fn framed(&mut self, input: &Input, length_at: usize, events: &mut Vec<Event>) -> Step {
        input.framed_ignored(length_at, 2, events)
    }
}

/// Reads a command of ESC/POS that has an `ESC` before it here: the reader gets the command
/// without the `ESC`, and anything it reports as unknown is given back with it.
fn without_esc(
    machine: &mut Machine,
    input: &Input,
    events: &mut Vec<Event>,
    read: impl FnOnce(&mut Machine, &Input, &mut Vec<Event>) -> Step,
) -> Step {
    let inner = Input {
        bytes: &input.bytes[1..],
        offset: input.offset + 1,
        at_end: input.at_end,
    };
    let first = events.len();
    let step = read(machine, &inner, events);
    for event in &mut events[first..] {
        if let Event::Unknown { offset, bytes } = event {
            *offset -= 1;
            bytes.insert(0, ESC);
        }
    }
    match step {
        Step::Consumed(length) => Step::Consumed(length + 1),
        Step::Incomplete => Step::Incomplete,
    }
}
