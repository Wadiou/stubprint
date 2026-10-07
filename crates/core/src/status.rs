/// How much paper is left, as the paper sensors tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    Adequate,
    NearEnd,
    End,
}

/// What went wrong, if something did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The cutter is stuck.
    Cutter,
    /// A fault that does not go away.
    Unrecoverable,
    /// A fault that goes away by itself, such as a print head that is too hot, or when the
    /// program asks for a recovery with `DLE ENQ`.
    Recoverable,
}

/// What a printer would say about itself when it is asked. A program that talks to a printer
/// reads these answers, and sets this to see how the program behaves when the paper runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrinterStatus {
    pub online: bool,
    pub cover_open: bool,
    pub paper: Paper,
    /// The level of pin 3 of the cash drawer connector.
    pub drawer_open: bool,
    /// The paper feed button is held.
    pub feed_button: bool,
    pub fault: Option<Fault>,
}

impl Default for PrinterStatus {
    fn default() -> Self {
        Self {
            online: true,
            cover_open: false,
            paper: Paper::Adequate,
            drawer_open: false,
            feed_button: false,
            fault: None,
        }
    }
}

/// Bits 1 and 4 are always on in a real time status byte, so a program can tell it from print data.
const FIXED: u8 = 0x12;

impl PrinterStatus {
    /// The answer to `DLE EOT n`, for n from 1 to 4.
    pub(crate) fn real_time(&self, n: u8) -> Option<u8> {
        let mut byte = FIXED;
        match n {
            // Printer status.
            1 => {
                if self.drawer_open {
                    byte |= 0x04;
                }
                if !self.online {
                    byte |= 0x08;
                }
                if self.feed_button {
                    byte |= 0x40;
                }
            }
            // Why the printer is off line.
            2 => {
                if self.cover_open {
                    byte |= 0x04;
                }
                if self.feed_button {
                    byte |= 0x08;
                }
                if self.paper == Paper::End {
                    byte |= 0x20;
                }
                if self.fault.is_some() {
                    byte |= 0x40;
                }
            }
            // The error.
            3 => match self.fault {
                Some(Fault::Cutter) => byte |= 0x08,
                Some(Fault::Unrecoverable) => byte |= 0x20,
                Some(Fault::Recoverable) => byte |= 0x40,
                None => {}
            },
            // The roll paper sensor.
            4 => match self.paper {
                Paper::Adequate => {}
                Paper::NearEnd => byte |= 0x0c,
                Paper::End => byte |= 0x0c | 0x60,
            },
            _ => return None,
        }
        Some(byte)
    }

    /// The answer to `GS r n`: 1 or 49 for the paper sensor, 2 or 50 for the cash drawer.
    pub(crate) fn transmit(&self, n: u8) -> Option<u8> {
        match n {
            1 | 49 => Some(match self.paper {
                Paper::Adequate => 0,
                Paper::NearEnd => 0x03,
                Paper::End => 0x03 | 0x0c,
            }),
            2 | 50 => Some(u8::from(self.drawer_open)),
            _ => None,
        }
    }

    /// The four bytes that `GS a` sends, and sends again whenever something they say changes.
    pub(crate) fn automatic(&self) -> [u8; 4] {
        let mut first = 0x10;
        if self.drawer_open {
            first |= 0x04;
        }
        if !self.online {
            first |= 0x08;
        }
        if self.cover_open {
            first |= 0x20;
        }
        if self.feed_button {
            first |= 0x40;
        }

        let second = match self.fault {
            Some(Fault::Cutter) => 0x08,
            Some(Fault::Unrecoverable) => 0x20,
            Some(Fault::Recoverable) => 0x40,
            None => 0,
        };

        let third = match self.paper {
            Paper::Adequate => 0,
            Paper::NearEnd => 0x03,
            Paper::End => 0x03 | 0x0c,
        };
        [first, second, third, 0]
    }

    /// Which of the things `GS a n` was asked to watch differ between two statuses.
    pub(crate) fn changed(&self, other: &PrinterStatus, mask: u8) -> bool {
        (mask & 0x01 != 0 && self.drawer_open != other.drawer_open)
            || (mask & 0x02 != 0
                && (self.online != other.online
                    || self.cover_open != other.cover_open
                    || self.feed_button != other.feed_button))
            || (mask & 0x04 != 0 && self.fault != other.fault)
            || (mask & 0x08 != 0 && self.paper != other.paper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_healthy_printer_answers_with_the_fixed_bits_only() {
        let status = PrinterStatus::default();
        for n in 1..=4 {
            assert_eq!(status.real_time(n), Some(0x12), "DLE EOT {n}");
        }
        assert_eq!(status.real_time(5), None);
        assert_eq!(status.transmit(1), Some(0));
        assert_eq!(status.transmit(2), Some(0));
        assert_eq!(status.automatic(), [0x10, 0, 0, 0]);
    }

    #[test]
    fn every_answer_can_be_told_from_print_data() {
        let status = PrinterStatus {
            online: false,
            cover_open: true,
            paper: Paper::End,
            drawer_open: true,
            feed_button: true,
            fault: Some(Fault::Unrecoverable),
        };
        for n in 1..=4 {
            let byte = status.real_time(n).unwrap();
            // Bits 0 and 7 are off and bits 1 and 4 are on.
            assert_eq!(byte & 0b1001_0011, 0b0001_0010, "DLE EOT {n}: {byte:08b}");
        }
    }

    #[test]
    fn each_status_says_what_it_is_for() {
        let paper_out = PrinterStatus {
            paper: Paper::End,
            ..PrinterStatus::default()
        };
        assert_eq!(paper_out.real_time(4), Some(0x12 | 0x0c | 0x60));
        assert_eq!(paper_out.real_time(2), Some(0x12 | 0x20));
        assert_eq!(paper_out.transmit(1), Some(0x0f));

        let near_end = PrinterStatus {
            paper: Paper::NearEnd,
            ..PrinterStatus::default()
        };
        assert_eq!(near_end.real_time(4), Some(0x12 | 0x0c));
        assert_eq!(near_end.transmit(1), Some(0x03));

        let open = PrinterStatus {
            online: false,
            cover_open: true,
            ..PrinterStatus::default()
        };
        assert_eq!(open.real_time(1), Some(0x12 | 0x08));
        assert_eq!(open.real_time(2), Some(0x12 | 0x04));
        assert_eq!(open.automatic(), [0x10 | 0x08 | 0x20, 0, 0, 0]);

        let hot = PrinterStatus {
            fault: Some(Fault::Recoverable),
            ..PrinterStatus::default()
        };
        assert_eq!(hot.real_time(3), Some(0x12 | 0x40));
        assert_eq!(hot.automatic()[1], 0x40);

        let drawer = PrinterStatus {
            drawer_open: true,
            ..PrinterStatus::default()
        };
        assert_eq!(drawer.real_time(1), Some(0x12 | 0x04));
        assert_eq!(drawer.transmit(2), Some(1));
    }

    #[test]
    fn only_the_watched_changes_count() {
        let before = PrinterStatus::default();
        let out = PrinterStatus {
            paper: Paper::End,
            ..before
        };
        assert!(before.changed(&out, 0x08));
        assert!(!before.changed(&out, 0x01 | 0x02 | 0x04));
        assert!(!before.changed(&before, 0xff));
    }
}
