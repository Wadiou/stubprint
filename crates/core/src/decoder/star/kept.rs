//! What a Star printer keeps when `ESC @` or `CAN` sets the commands back: how it reports its
//! status, the counters that report it, and how long it drives a drawer or a buzzer.

#[derive(Debug, Clone)]
pub(in crate::decoder) struct Kept {
    /// Set by `ESC RS a`. Bit 0 is the automatic status (ASB), sent whenever the status changes,
    /// and bit 1 the one sent when a connection is made, which has no connection to make here.
    reporting: u8,
    /// How many `ETB` commands there were, in five bits.
    etb: u8,
    /// An `ETB` that the status has not told yet.
    etb_pending: bool,
    /// How many print jobs were counted by `ESC GS ETX`, which wraps around at 256.
    print_end: u8,
    /// How long the printer waits for data before it takes the connection to be lost, in seconds.
    timeout: u8,
    /// How long, in milliseconds, each of the four drawer terminals is driven, and rests.
    drawers: [(u16, u16); 4],
    /// The same for the two buzzer terminals, in units of 20 milliseconds.
    buzzers: [(u8, u8); 2],
}

impl Default for Kept {
    fn default() -> Self {
        Self {
            reporting: 0,
            etb: 0,
            etb_pending: false,
            print_end: 0,
            timeout: 3,
            drawers: [(200, 200); 4],
            buzzers: [(0, 0); 2],
        }
    }
}

impl Kept {
    pub fn reports_by_itself(&self) -> bool {
        self.reporting & 1 != 0
    }

    pub fn set_reporting(&mut self, reporting: u8) {
        self.reporting = reporting;
    }

    /// An `ETB` was executed: the counter counts it, and the next status says so.
    pub fn count_etb(&mut self) {
        self.etb = (self.etb + 1) & 0x1f;
        self.etb_pending = true;
    }

    pub fn etb_counter(&self) -> u8 {
        self.etb
    }

    pub fn etb_pending(&self) -> bool {
        self.etb_pending
    }

    /// The status told about the `ETB`.
    pub fn etb_told(&mut self) {
        self.etb_pending = false;
    }

    /// `CAN` and `ESC RS E` clear the `ETB` counter, and with it what the status would tell.
    pub fn clear_counters(&mut self) {
        self.etb = 0;
        self.etb_pending = false;
    }

    /// A job was counted by `ESC GS ETX`.
    pub fn count_print_end(&mut self) {
        self.print_end = self.print_end.wrapping_add(1);
    }

    pub fn print_end(&self) -> u8 {
        self.print_end
    }

    pub fn clear_print_end(&mut self) {
        self.print_end = 0;
    }

    pub fn timeout(&self) -> u8 {
        self.timeout
    }

    pub fn set_timeout(&mut self, seconds: u8) {
        self.timeout = seconds;
    }

    pub fn drawer(&self, terminal: u8) -> (u16, u16) {
        self.drawers[usize::from(terminal.min(3))]
    }

    pub fn set_drawer(&mut self, terminal: u8, on_ms: u16, off_ms: u16) {
        self.drawers[usize::from(terminal.min(3))] = (on_ms, off_ms);
    }

    pub fn buzzer(&self, terminal: u8) -> (u8, u8) {
        self.buzzers[usize::from(terminal.min(1))]
    }

    pub fn set_buzzer(&mut self, terminal: u8, on: u8, off: u8) {
        self.buzzers[usize::from(terminal.min(1))] = (on, off);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_etb_counter_has_five_bits_and_is_told_once() {
        let mut kept = Kept::default();
        for _ in 0..33 {
            kept.count_etb();
        }
        assert_eq!(kept.etb_counter(), 1);
        assert!(kept.etb_pending());
        kept.etb_told();
        assert!(!kept.etb_pending());
        kept.count_etb();
        kept.clear_counters();
        assert_eq!((kept.etb_counter(), kept.etb_pending()), (0, false));
    }

    #[test]
    fn the_print_end_counter_wraps_around() {
        let mut kept = Kept::default();
        for _ in 0..257 {
            kept.count_print_end();
        }
        assert_eq!(kept.print_end(), 1);
        kept.clear_print_end();
        assert_eq!(kept.print_end(), 0);
    }

    #[test]
    fn a_drawer_is_driven_for_200_milliseconds_until_told_otherwise() {
        let mut kept = Kept::default();
        assert_eq!(kept.drawer(1), (200, 200));
        kept.set_drawer(0, 100, 50);
        assert_eq!(kept.drawer(0), (100, 50));
        assert_eq!(kept.drawer(1), (200, 200));
    }
}
