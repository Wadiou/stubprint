//! How a Star printer says its status: the byte of `ENQ` and `EOT`, and the nine bytes of the
//! automatic status (ASB) that `ESC ACK SOH` and `ESC RS a` ask for.

use super::{Fault, Paper, PrinterStatus};

/// The automatic status of the TSP650 and later models: a first header that says there are nine
/// bytes in all, a second that says it is version 3, and seven bytes of printer status.
const ASB_HEADERS: [u8; 2] = [0x23, 0x06];

impl PrinterStatus {
    /// The printer is off line when it was set so, and when the cover is open, the paper has run out
    /// or there is a fault.
    fn star_offline(&self) -> bool {
        !self.online || self.cover_open || self.paper == Paper::End || self.fault.is_some()
    }

    /// The answer to `ENQ`: bit 3 says the paper has run out and bit 2 that there is another error,
    /// which is a fault or an open cover. Bit 4 is 0, which tells it from the answer to `EOT`.
    pub(crate) fn star_enq(&self) -> u8 {
        let mut byte = 0;
        if self.paper == Paper::End {
            byte |= 0x08;
        }
        if self.cover_open || self.fault.is_some() {
            byte |= 0x04;
        }
        byte
    }

    /// The answer to `EOT`: bit 4 is always 1, bit 3 says the paper has run out and bit 2 that the
    /// paper is near its end.
    pub(crate) fn star_eot(&self) -> u8 {
        let mut byte = 0x10;
        match self.paper {
            Paper::Adequate => {}
            Paper::NearEnd => byte |= 0x04,
            Paper::End => byte |= 0x0c,
        }
        byte
    }

    /// The nine bytes of the automatic status. `etb_pending` is set between an `ETB` and the first
    /// status that reports it, and `etb_counter` is the five bit count of `ETB` commands.
    pub(crate) fn star_asb(&self, etb_pending: bool, etb_counter: u8) -> [u8; 9] {
        let mut first = 0;
        if self.feed_button {
            first |= 0x40;
        }
        if self.cover_open {
            first |= 0x20;
        }
        if self.star_offline() {
            first |= 0x08;
        }
        if self.drawer_open {
            first |= 0x04;
        }
        if etb_pending {
            first |= 0x02;
        }

        let second = match self.fault {
            Some(Fault::Recoverable) => 0x40,
            Some(Fault::Unrecoverable) => 0x20,
            Some(Fault::Cutter) => 0x08,
            None => 0,
        };

        let paper = match self.paper {
            Paper::Adequate => 0,
            Paper::NearEnd => 0x04,
            Paper::End => 0x08 | 0x04,
        };

        // The count is spread over bits 1, 2, 3, 5 and 6, so no byte of the status looks like the
        // first header, which has bit 0 on.
        let counter = [(0, 0x02), (1, 0x04), (2, 0x08), (3, 0x20), (4, 0x40)]
            .into_iter()
            .filter(|(bit, _)| etb_counter & (1 << bit) != 0)
            .fold(0, |byte, (_, mask)| byte | mask);

        [
            ASB_HEADERS[0],
            ASB_HEADERS[1],
            first,
            second,
            0,
            paper,
            0,
            counter,
            0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_healthy_printer_answers_with_nothing_set() {
        let status = PrinterStatus::default();
        assert_eq!(status.star_enq(), 0);
        assert_eq!(status.star_eot(), 0x10);
        assert_eq!(status.star_asb(false, 0), [0x23, 0x06, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn the_paper_and_the_cover_are_told_in_each_answer() {
        let out = PrinterStatus {
            paper: Paper::End,
            cover_open: true,
            ..PrinterStatus::default()
        };
        assert_eq!(out.star_enq(), 0x0c);
        assert_eq!(out.star_eot(), 0x1c);
        let asb = out.star_asb(false, 0);
        assert_eq!(asb[2], 0x20 | 0x08);
        assert_eq!(asb[5], 0x0c);

        let near = PrinterStatus {
            paper: Paper::NearEnd,
            ..PrinterStatus::default()
        };
        assert_eq!(near.star_eot(), 0x14);
        assert_eq!(near.star_asb(false, 0)[5], 0x04);
    }

    #[test]
    fn each_fault_has_its_own_bit() {
        let asb = |fault| {
            PrinterStatus {
                fault: Some(fault),
                ..PrinterStatus::default()
            }
            .star_asb(false, 0)[3]
        };
        assert_eq!(asb(Fault::Recoverable), 0x40);
        assert_eq!(asb(Fault::Unrecoverable), 0x20);
        assert_eq!(asb(Fault::Cutter), 0x08);
    }

    #[test]
    fn the_etb_counter_is_spread_over_five_bits() {
        let status = PrinterStatus::default();
        assert_eq!(status.star_asb(false, 0b00001)[7], 0x02);
        assert_eq!(status.star_asb(false, 0b00010)[7], 0x04);
        assert_eq!(status.star_asb(false, 0b00100)[7], 0x08);
        assert_eq!(status.star_asb(false, 0b01000)[7], 0x20);
        assert_eq!(status.star_asb(false, 0b10000)[7], 0x40);
        assert_eq!(status.star_asb(false, 0b11111)[7], 0x6e);
        assert_eq!(status.star_asb(true, 0)[2], 0x02);
    }
}
