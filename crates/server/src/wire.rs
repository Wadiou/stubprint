//! What the API says, as JSON.

use serde::Serialize;
use stubprint_core::{Dialect, Event, PrinterStatus, Receipt, Snapshot, Traced};
use utoipa::ToSchema;

use crate::hub::Stored;
use crate::printer::VirtualPrinter;

/// How many printed lines the short form of a receipt shows.
const PREVIEW_LINES: usize = 3;

/// A receipt in a list.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReceiptSummary {
    /// The number of the receipt, which is never used again.
    pub id: u64,
    /// The name of the printer that printed it.
    pub printer: String,
    pub dialect: Dialect,
    /// When it was cut, in milliseconds since 1970.
    pub at: u64,
    /// How many bytes made it.
    pub bytes: usize,
    /// How many events, which are the commands it holds.
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
    /// What the API calls the printer.
    pub name: String,
    pub dialect: Dialect,
    /// The printer model, which decides the code pages, the columns and the answers.
    pub profile: String,
    /// The address of its TCP port.
    pub listen: String,
    pub status: PrinterStatus,
    /// How many programs are connected to it now.
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

/// One command of a job: its bytes, what it asked for, and what the printer believed after it.
#[derive(Debug, Serialize, ToSchema)]
pub struct TraceItem<'a> {
    /// Which event of the receipt it is.
    pub index: usize,
    /// Where its bytes start and end. A command that made several events gives each the same.
    pub start: usize,
    pub end: usize,
    pub event: &'a Event,
    pub state: &'a Snapshot,
}

/// The commands of a receipt, in order, with the place of their bytes in it.
pub fn trace_of(receipt: &Receipt) -> Vec<TraceItem<'_>> {
    let mut start = 0;
    receipt
        .events()
        .iter()
        .zip(receipt.steps())
        .enumerate()
        .map(|(index, (event, step))| {
            let item = TraceItem {
                index,
                start,
                end: step.end,
                event,
                state: &step.state,
            };
            start = step.end;
            item
        })
        .collect()
}

/// The commands of a stream that was read as one, from the first byte.
pub fn trace_of_stream(traced: &[Traced]) -> Vec<TraceItem<'_>> {
    let mut start = 0;
    traced
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let out = TraceItem {
                index,
                start,
                end: item.end,
                event: &item.event,
                state: &item.state,
            };
            start = item.end;
            out
        })
        .collect()
}
