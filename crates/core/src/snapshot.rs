use serde::Serialize;

use crate::event::{Align, Style};
use crate::layout::Layout;

/// What a printer believed after a command: how the next text is drawn, how bytes are read, and
/// where the paper settings are. It is for debugging, to see why a line came out as it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Snapshot {
    pub style: Style,
    pub align: Align,
    /// The number the last code page command sent, and the code page it means on this model.
    pub code_page_number: u8,
    pub code_page: Option<String>,
    /// The international character set, 0 for American.
    pub international: u8,
    /// The text is read as UTF-8.
    pub utf8: bool,
    /// The characters the program drew are used in place of the font's.
    pub user_characters: bool,
    /// Two byte characters are read.
    pub kanji: bool,
    pub page_mode: bool,
    pub layout: Layout,
}
