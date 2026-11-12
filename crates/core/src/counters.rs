//! What a printer counts about its own work, which a program can ask it for.

/// The maintenance counters of a printer. A program reads them to know when a printer needs a
/// service, with `GS g 2` on an Epson printer. The printer keeps them for as long as it lives, so
/// they come from the owner of the printer and not from one connection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Counter 20: how many lines the paper was fed, by line feeds and by feeding lines.
    pub line_feeds: u64,
    /// Counter 50: how many times the cutter cut the paper.
    pub cuts: u64,
}

impl Counters {
    /// The value of the counter with this number, or `None` when the printer has no such counter.
    pub fn get(&self, number: u16) -> Option<u64> {
        match number {
            20 => Some(self.line_feeds),
            50 => Some(self.cuts),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counter_is_found_by_its_number_and_the_others_are_not() {
        let counters = Counters {
            line_feeds: 7,
            cuts: 2,
        };
        assert_eq!(counters.get(20), Some(7));
        assert_eq!(counters.get(50), Some(2));
        assert_eq!(counters.get(21), None);
    }
}
