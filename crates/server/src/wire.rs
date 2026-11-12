//! What the API says, as JSON.

use serde::Serialize;
use stubprint_core::{Dialect, Event, PrinterStatus, Profile, Receipt, Snapshot, Traced};
use utoipa::ToSchema;

use crate::hub::Stored;
use crate::printer::VirtualPrinter;
use crate::samples::{Sample, Variant};

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
    /// The printer model it was printed on, as `GET /api/profiles` lists them.
    pub profile: String,
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
            profile: stored.profile.name.clone(),
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
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PrinterInfo {
    /// What the API calls the printer.
    pub name: String,
    pub dialect: Dialect,
    /// The printer model, which decides the code pages, the columns and the answers.
    pub profile: String,
    /// The address of its TCP port.
    pub listen: String,
    pub status: PrinterStatus,
    /// How many bytes it holds while it cannot print, such as with its cover open. 0 means it holds
    /// nothing and prints whatever it receives.
    pub hold_offline: usize,
    /// How many bytes the network keeps for what a program sends before the printer reads it. 0
    /// leaves it to the system, which keeps a lot.
    pub receive_buffer: usize,
    /// How many bytes it holds now because it cannot print.
    pub held: usize,
    /// How many programs are connected to it now.
    pub connections: usize,
}

impl PrinterInfo {
    pub fn of(printer: &VirtualPrinter) -> Self {
        Self {
            name: printer.name.clone(),
            dialect: printer.dialect,
            profile: printer.profile().name.clone(),
            listen: printer.listen.to_string(),
            status: printer.status(),
            hold_offline: printer.hold_offline(),
            receive_buffer: printer.receive_buffer(),
            held: printer.held(),
            connections: printer.connections(),
        }
    }
}

/// A printer model that a printer can be set to.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProfileInfo {
    /// The name to give as the `profile` of a printer.
    pub name: String,
    /// The maker, which is empty for the generic models.
    pub vendor: String,
    /// The model as the maker names it.
    pub model: String,
    /// How many columns each font has, font A first. A font the model does not have has 0.
    pub columns: Vec<u8>,
    /// How wide the paper prints, in dots, when the model says.
    pub width_dots: Option<u32>,
    /// How wide the paper prints, in millimetres, when the model says.
    pub width_mm: Option<f64>,
    /// How many dots make an inch, when the model says.
    pub dpi: Option<u32>,
    /// How many code pages `ESC t` can choose.
    pub code_pages: usize,
    /// Links to the manuals of the model.
    pub manuals: Vec<String>,
    /// What the model can do, such as `qrCode`, `pdf417Code` or `starCommands`.
    pub features: Vec<String>,
}

impl ProfileInfo {
    pub fn of(profile: &Profile) -> Self {
        Self {
            name: profile.name.clone(),
            vendor: profile.vendor.clone(),
            model: profile.identity.model.clone(),
            columns: profile.columns.clone(),
            width_dots: profile.width_dots,
            width_mm: profile.width_mm,
            dpi: profile.dpi,
            code_pages: profile.code_page_count(),
            manuals: profile.manuals.clone(),
            features: profile.features().into_iter().map(str::to_owned).collect(),
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
    /// Where it ends, counted in bytes from the start of the receipt.
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

/// One way a printing library wrote a sample.
#[derive(Debug, Serialize, ToSchema)]
pub struct VariantInfo {
    /// The library that wrote the bytes, such as `python-escpos`.
    pub library: String,
    /// The version of that library, which is exactly the one that wrote these bytes.
    pub version: String,
    /// The language the library is for: `python` or `node`.
    pub language: String,
    /// The command language the bytes are in. Print them to a printer set to it.
    pub dialect: Dialect,
    /// How many bytes it is.
    pub bytes: usize,
}

impl VariantInfo {
    pub fn of(variant: &Variant) -> Self {
        Self {
            library: variant.library.clone(),
            version: variant.version.clone(),
            language: variant.language.clone(),
            dialect: variant.dialect,
            bytes: variant.bytes.len(),
        }
    }
}

/// A ready made receipt, in a list.
#[derive(Debug, Serialize, ToSchema)]
pub struct SampleSummary {
    /// What it is called in addresses, such as `grocery`. It never says how it was made.
    pub id: String,
    /// A name to show.
    pub title: String,
    /// What the sample shows and what to look at in it.
    pub about: String,
    /// The printer model it was written for, as `GET /api/profiles` lists them. It decides the
    /// width of a line and the code pages the bytes assume.
    pub profile: String,
    /// Short words for what it holds, to filter by.
    pub tags: Vec<String>,
    /// Each library that wrote it, with its bytes.
    pub variants: Vec<VariantInfo>,
}

impl SampleSummary {
    pub fn of(sample: &Sample) -> Self {
        Self {
            id: sample.id.clone(),
            title: sample.title.clone(),
            about: sample.about.clone(),
            profile: sample.profile.clone(),
            tags: sample.tags.clone(),
            variants: sample.variants.iter().map(VariantInfo::of).collect(),
        }
    }
}

/// A ready made receipt with what it says as data.
#[derive(Debug, Serialize, ToSchema)]
pub struct SampleDetail {
    #[serde(flatten)]
    pub summary: SampleSummary,
    /// What is printed, block by block, in the form every library was given.
    #[schema(value_type = Object)]
    pub receipt: serde_json::Value,
    /// What a decoder must find on the paper, from what the receipt asks for and not from reading
    /// any library's bytes: the lines, the codes, the pictures and the cuts.
    #[schema(value_type = Object)]
    pub expect: serde_json::Value,
}

impl SampleDetail {
    pub fn of(sample: &Sample) -> Self {
        Self {
            summary: SampleSummary::of(sample),
            receipt: sample.receipt.clone(),
            expect: sample.expect.clone(),
        }
    }
}
