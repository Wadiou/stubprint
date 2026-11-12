//! What the server remembers between runs, when it is given a file for it: the printers, each with
//! its settings and what it reports. The receipts and the connections are not kept.

use std::io;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use stubprint_core::{Dialect, PrinterStatus, Profile};

use crate::config::PrinterConfig;
use crate::printer::VirtualPrinter;

#[derive(Debug, Serialize, Deserialize)]
struct File {
    printers: Vec<Saved>,
}

/// One printer as it is written in the file.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Saved {
    name: String,
    listen: String,
    dialect: String,
    profile: String,
    /// Files written before the size was kept by this name have none, and get the default.
    #[serde(default = "default_hold")]
    hold: usize,
    /// Files written before it was kept leave it to the system.
    #[serde(default)]
    receive_buffer: usize,
    pub(crate) status: PrinterStatus,
}

fn default_hold() -> usize {
    crate::config::DEFAULT_HOLD
}

impl Saved {
    fn of(printer: &VirtualPrinter) -> Self {
        Self {
            name: printer.name.clone(),
            listen: printer.listen.to_string(),
            dialect: printer.dialect.name().to_string(),
            profile: printer.profile().name.clone(),
            hold: printer.hold_offline(),
            receive_buffer: printer.receive_buffer(),
            status: printer.status(),
        }
    }

    /// What the printer is called.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// The settings to start the printer with. A model that the database no longer has is the
    /// generic one.
    pub(crate) fn config(&self) -> Result<PrinterConfig, String> {
        Ok(PrinterConfig {
            name: self.name.clone(),
            listen: self
                .listen
                .parse()
                .map_err(|_| format!("`{}` is not an address", self.listen))?,
            dialect: Dialect::from_name(&self.dialect)
                .ok_or_else(|| format!("`{}` is not a dialect", self.dialect))?,
            profile: Profile::named(&self.profile).unwrap_or_else(Profile::generic),
            hold_offline: self.hold,
            receive_buffer: self.receive_buffer,
        })
    }
}

/// The printers of the file, or nothing when there is no file yet.
pub(crate) fn read(path: &Path) -> io::Result<Option<Vec<Saved>>> {
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
            format!("{} is not a state file: {error}", path.display()),
        )
    })?;
    Ok(Some(file.printers))
}

/// Writes the printers. The file appears whole or not at all.
pub(crate) fn write(path: &Path, printers: &[Arc<VirtualPrinter>]) -> io::Result<()> {
    let file = File {
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
