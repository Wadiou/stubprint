//! The config file: where the API listens, and the printers with their settings and status.
//! The server reads it when it starts and rewrites it after every change, so it always matches what
//! is running. It can also be written by hand: anything left out gets its default. Receipts and
//! connections are not saved.

use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use stubprint_core::{Dialect, Fault, Paper, PrinterStatus, Profile};

use crate::config::PrinterConfig;
use crate::printer::VirtualPrinter;

/// The whole file, as JSON.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    http: Option<SocketAddr>,
    #[serde(default)]
    printers: Vec<Saved>,
}

/// What a config file contained: the saved API address, if there is one, and the saved printers.
pub(crate) struct Kept {
    pub(crate) http: Option<SocketAddr>,
    pub(crate) printers: Vec<Saved>,
}

/// One printer as saved in the file. Anything left out gets its default when it is read.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    #[serde(default = "default_name")]
    name: String,
    listen: Option<String>,
    dialect: Option<String>,
    profile: Option<String>,
    /// Older files call this `hold` or `hold_offline`.
    #[serde(alias = "hold", alias = "hold_offline")]
    printer_buffer: Option<usize>,
    /// Older files call this `receive_buffer`.
    #[serde(alias = "receive_buffer")]
    network_buffer: Option<usize>,
    status: Option<Reported>,
}

/// A printer's status as saved. Anything missing, in a file written by hand, is what a healthy
/// printer reports.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reported {
    online: Option<bool>,
    cover_open: Option<bool>,
    paper: Option<Paper>,
    drawer_open: Option<bool>,
    feed_button: Option<bool>,
    fault: Option<Fault>,
}

impl Reported {
    fn of(status: PrinterStatus) -> Self {
        Self {
            online: Some(status.online),
            cover_open: Some(status.cover_open),
            paper: Some(status.paper),
            drawer_open: Some(status.drawer_open),
            feed_button: Some(status.feed_button),
            fault: status.fault,
        }
    }

    fn status(&self) -> PrinterStatus {
        let fine = PrinterStatus::default();
        PrinterStatus {
            online: self.online.unwrap_or(fine.online),
            cover_open: self.cover_open.unwrap_or(fine.cover_open),
            paper: self.paper.unwrap_or(fine.paper),
            drawer_open: self.drawer_open.unwrap_or(fine.drawer_open),
            feed_button: self.feed_button.unwrap_or(fine.feed_button),
            fault: self.fault,
        }
    }
}

fn default_name() -> String {
    PrinterConfig::default().name
}

impl Saved {
    fn of(printer: &VirtualPrinter) -> Self {
        Self {
            name: printer.name.clone(),
            listen: Some(printer.listen.to_string()),
            dialect: Some(printer.dialect.name().to_string()),
            profile: Some(printer.profile().name.clone()),
            printer_buffer: Some(printer.printer_buffer()),
            network_buffer: Some(printer.network_buffer()),
            status: Some(Reported::of(printer.status())),
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// The status it had. A healthy printer (online, nothing wrong) when the file has none.
    pub(crate) fn status(&self) -> PrinterStatus {
        self.status
            .as_ref()
            .map_or_else(PrinterStatus::default, Reported::status)
    }

    /// The settings to start the printer with. Anything the file leaves out gets its default, and a
    /// profile name that no longer exists becomes the generic profile.
    pub(crate) fn config(&self) -> Result<PrinterConfig, String> {
        let mut printer = PrinterConfig {
            name: self.name.clone(),
            ..PrinterConfig::default()
        };
        if let Some(listen) = &self.listen {
            printer.listen = listen
                .parse()
                .map_err(|_| format!("`{listen}` is not an address"))?;
        }
        if let Some(dialect) = &self.dialect {
            printer.dialect = Dialect::from_name(dialect)
                .ok_or_else(|| format!("`{dialect}` is not a dialect"))?;
        }
        if let Some(profile) = &self.profile {
            printer.profile = Profile::named(profile).unwrap_or_else(Profile::generic);
        }
        if let Some(bytes) = self.printer_buffer {
            printer.printer_buffer = bytes;
        }
        if let Some(bytes) = self.network_buffer {
            printer.network_buffer = bytes;
        }
        Ok(printer)
    }
}

/// Reads the config file. `None` means it does not exist yet.
pub(crate) fn read(path: &Path) -> io::Result<Option<Kept>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("cannot read {}: {error}", path.display()),
            ))
        }
    };
    let file: File = serde_json::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is not a config file: {error}", path.display()),
        )
    })?;
    Ok(Some(Kept {
        http: file.http,
        printers: file.printers,
    }))
}

/// Writes the config file. It is written to a temporary file and then renamed, so it is never half
/// written.
pub(crate) fn write(
    path: &Path,
    http: SocketAddr,
    printers: &[Arc<VirtualPrinter>],
) -> io::Result<()> {
    let file = File {
        http: Some(http),
        printers: printers.iter().map(|printer| Saved::of(printer)).collect(),
    };
    let text = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
    if let Some(folder) = path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        std::fs::create_dir_all(folder)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
}
