//! Reading bytes: the bytes of a command that has arrived, what a command says it needs, and the
//! control bytes and limits that every command language shares.

use crate::event::Event;

pub(super) const ESC: u8 = 0x1b;
pub(super) const GS: u8 = 0x1d;
pub(super) const FS: u8 = 0x1c;
pub(super) const LF: u8 = 0x0a;
pub(super) const CR: u8 = 0x0d;
pub(super) const DLE: u8 = 0x10;
pub(super) const HT: u8 = 0x09;
pub(super) const FF: u8 = 0x0c;
pub(super) const CAN: u8 = 0x18;
pub(super) const BEL: u8 = 0x07;
pub(super) const FIRST_TEXT_BYTE: u8 = 0x20;

/// The most bytes one command may take, so a header that promises gigabytes cannot make the decoder
/// wait for them and grow without end. A command that asks for more is reported as unknown.
pub(super) const MAX_COMMAND_BYTES: usize = 8 * 1024 * 1024;
/// How far the data of a barcode may run before the byte that ends it.
pub(super) const MAX_BARCODE_BYTES: usize = 4096;
/// Text with no line feed is reported in pieces of this many bytes, as a printer would wrap it.
pub(super) const MAX_LINE_BYTES: usize = 4096;
/// How many bytes of pictures a printer remembers, so a stream cannot fill the memory with them.
pub(super) const MAX_STORED_BYTES: usize = 32 * 1024 * 1024;

/// Not enough bytes yet to know what the command is, or to read all of it.
pub(super) enum Step {
    Consumed(usize),
    Incomplete,
}

/// How much of a command with a length is there.
pub(super) enum Frame {
    /// The bytes that say the length have not all arrived.
    Need,
    /// The command says it is longer than a command may be.
    TooBig,
    /// The whole command, this many bytes long, has arrived.
    Whole(usize),
    /// The length is known, and the command is not all here yet.
    Waiting,
}

/// The bytes being read, where they started in the stream, and whether the stream has ended.
pub(super) struct Input<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) offset: usize,
    pub(super) at_end: bool,
}

impl Input<'_> {
    /// Reads a list of stops that starts after the two bytes of a command, such as `ESC D`: numbers
    /// that go up, ended by a NUL. A byte that breaks the order, or the one after the last stop
    /// there is room for, ends the list and is left to be read as data. Gives the stops and how
    /// many bytes the command took, or nothing if the list is not all here.
    pub(super) fn stops(&self, most: usize) -> Option<(Vec<u8>, usize)> {
        let mut stops: Vec<u8> = Vec::new();
        let mut index = 2;
        loop {
            let next = self.get(index)?;
            if next == 0 {
                index += 1;
                break;
            }
            if stops.last().is_some_and(|last| next <= *last) || stops.len() == most {
                break;
            }
            stops.push(next);
            index += 1;
        }
        Some((stops, index))
    }
    pub(super) fn get(&self, index: usize) -> Option<u8> {
        self.bytes.get(index).copied()
    }

    /// An unknown command is reported once, with the control bytes that follow it. A real printer
    /// ignores those too. The run is kept open until a byte that ends it arrives, so the result
    /// does not depend on where the stream was cut.
    pub(super) fn unknown_run(&self, events: &mut Vec<Event>) -> Step {
        let bytes = self.bytes;
        let mut end = if matches!(bytes[0], ESC | GS | FS) {
            2
        } else {
            1
        };
        if bytes.len() < end {
            return Step::Incomplete;
        }
        while end < bytes.len() && is_ignorable(bytes[end]) {
            end += 1;
        }
        if end == bytes.len() && !self.at_end {
            return Step::Incomplete;
        }
        events.push(Event::Unknown {
            offset: self.offset,
            bytes: bytes[..end].to_vec(),
        });
        Step::Consumed(end)
    }

    /// A command of a known length that is not read, reported as one unknown command.
    pub(super) fn fixed_unknown(&self, length: usize, events: &mut Vec<Event>) -> Step {
        if self.bytes.len() < length {
            return Step::Incomplete;
        }
        events.push(Event::Unknown {
            offset: self.offset,
            bytes: self.bytes[..length].to_vec(),
        });
        Step::Consumed(length)
    }

    /// How long a command is that has its length in the bytes after its name, such as
    /// `GS ( L pL pH data`. `length_at` is where the length starts and `width` how many bytes it has.
    pub(super) fn frame(&self, length_at: usize, width: usize) -> Frame {
        if self.bytes.len() < length_at + width {
            return Frame::Need;
        }
        let data = self.bytes[length_at..length_at + width]
            .iter()
            .rev()
            .fold(0usize, |total, byte| total * 256 + usize::from(*byte));
        let length = length_at + width + data;
        if length > MAX_COMMAND_BYTES {
            Frame::TooBig
        } else if self.bytes.len() < length {
            Frame::Waiting
        } else {
            Frame::Whole(length)
        }
    }

    /// A command with a length that is not read, reported as one unknown command, so its data is
    /// not taken for text or other commands.
    pub(super) fn framed_unknown(
        &self,
        length_at: usize,
        width: usize,
        events: &mut Vec<Event>,
    ) -> Step {
        match self.frame(length_at, width) {
            Frame::Need | Frame::Waiting => Step::Incomplete,
            Frame::TooBig => self.unknown_run(events),
            Frame::Whole(length) => {
                events.push(Event::Unknown {
                    offset: self.offset,
                    bytes: self.bytes[..length].to_vec(),
                });
                Step::Consumed(length)
            }
        }
    }

    /// A command with a length that has nothing to show, such as a setting of the printer's
    /// hardware, read and left alone.
    pub(super) fn framed_ignored(
        &self,
        length_at: usize,
        width: usize,
        events: &mut Vec<Event>,
    ) -> Step {
        match self.frame(length_at, width) {
            Frame::Need | Frame::Waiting => Step::Incomplete,
            Frame::TooBig => self.unknown_run(events),
            Frame::Whole(length) => Step::Consumed(length),
        }
    }
}

fn is_ignorable(byte: u8) -> bool {
    byte < FIRST_TEXT_BYTE && !matches!(byte, LF | CR | ESC | FS | GS)
}
