mod epson;
mod star;

use serde::{Deserialize, Serialize};

/// How much paper is left, as the paper sensors tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    #[default]
    Adequate,
    NearEnd,
    End,
}

/// What went wrong, if something did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PrinterStatus {
    /// The printer is on line and ready to print.
    pub online: bool,
    /// The cover is open.
    pub cover_open: bool,
    pub paper: Paper,
    /// The level of pin 3 of the cash drawer connector.
    pub drawer_open: bool,
    /// The paper feed button is held.
    pub feed_button: bool,
    /// What is wrong, when something is.
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

impl PrinterStatus {
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
