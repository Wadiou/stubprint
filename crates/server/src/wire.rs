//! What the API says, as JSON.

use receipt::{Dialect, Event, PrinterStatus};
use serde::Serialize;
use utoipa::ToSchema;

use crate::hub::Stored;
use crate::printer::VirtualPrinter;

/// How many printed lines the short form of a receipt shows.
const PREVIEW_LINES: usize = 3;

/// A receipt in a list.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReceiptSummary {
    pub id: u64,
    pub printer: String,
    pub dialect: Dialect,
    /// When it was cut, in milliseconds since 1970.
    pub at: u64,
    /// How many bytes made it.
    pub bytes: usize,
    pub events: usize,
    /// The first printed lines.
    pub preview: Vec<String>,
}

impl ReceiptSummary {
    pub fn of(stored: &Stored) -> Self {
        Self {
            id: stored.id,
            printer: stored.printer.name.clone(),
            dialect: stored.printer.dialect,
            at: stored.at,
            bytes: stored.receipt.raw().len(),
            events: stored.receipt.events().len(),
            preview: stored
                .receipt
                .texts()
                .into_iter()
                .take(PREVIEW_LINES)
                .map(str::to_string)
                .collect(),
        }
    }
}

/// A whole receipt: what it says and everything the printer was asked to do.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReceiptDetail<'a> {
    #[serde(flatten)]
    pub summary: ReceiptSummary,
    /// Every printed line.
    pub texts: Vec<&'a str>,
    pub events: &'a [Event],
}

impl<'a> ReceiptDetail<'a> {
    pub fn of(stored: &'a Stored) -> Self {
        Self {
            summary: ReceiptSummary::of(stored),
            texts: stored.receipt.texts(),
            events: stored.receipt.events(),
        }
    }
}

/// A printer, and how it is doing.
#[derive(Debug, Serialize, ToSchema)]
pub struct PrinterInfo {
    pub name: String,
    pub dialect: Dialect,
    pub profile: String,
    pub listen: String,
    pub status: PrinterStatus,
    pub connections: usize,
}

impl PrinterInfo {
    pub fn of(printer: &VirtualPrinter) -> Self {
        Self {
            name: printer.name.clone(),
            dialect: printer.dialect,
            profile: printer.profile.name.clone(),
            listen: printer.listen.to_string(),
            status: printer.status(),
            connections: printer.connections(),
        }
    }
}

/// Bytes as hexadecimal text.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
