use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use serde::Deserialize;
use stubprint_core::{Dialect, Profile};

use crate::settings::{Settings, SettingsChange, Unknown};

/// How the server starts: where the API listens, which printers it starts, and where it keeps
/// its state.
#[derive(Debug, Clone)]
pub struct Config {
    /// The address the API and the web page listen on. `None` means the one saved in the database,
    /// or that of [`DEFAULT_HTTP`] if there is none. What is set here is saved.
    pub host: Option<IpAddr>,
    /// The port the API and the web page listen on, with the same rule as `host`.
    pub port: Option<u16>,
    /// The printers from `--printer`, or just the default printer when there were none.
    pub printers: Vec<PrinterConfig>,
    /// A file to write the real addresses into once everything is listening, for a program that
    /// started the server on port 0. It is deleted when the server stops.
    pub ready: Option<PathBuf>,
    /// The database: the address of the API, the printers, the receipts and the connections. It is
    /// read when the server starts and written after every change. `None` keeps nothing, so it
    /// is all gone when the server stops.
    pub database: Option<PathBuf>,
    /// Delete what the database has when starting, and start from `printers`.
    pub reset: bool,
    /// True if `printers` came from `--printer`, false if it is only the default printer.
    /// A printer from `--printer` is added to the saved printers only when none of them has the
    /// same name. The saved version of a printer is never changed.
    pub given: bool,
}

/// One virtual printer. Its port decides the command language it reads, like the emulation
/// setting of a real printer.
#[derive(Debug, Clone)]
pub struct PrinterConfig {
    /// What the API calls it. Letters, digits, `-` and `_`.
    pub name: String,
    pub listen: SocketAddr,
    pub dialect: Dialect,
    pub profile: Profile,
    /// What it is set to: its buffers, its paper roll, its speed, its limits and its failures.
    pub settings: Settings,
}

/// The most programs that can be connected to one printer, whatever its settings say, so that a
/// program that opens connections without end cannot fill the memory.
pub const MAX_SOCKETS: usize = 64;

/// The `printer_buffer` of a printer when none is set, in bytes.
pub const DEFAULT_PRINTER_BUFFER: usize = 4096;

/// Where the API listens when neither `--host`, `--port` nor the database says.
pub const DEFAULT_HTTP: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 8080);

impl Default for Config {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            printers: vec![PrinterConfig::default()],
            ready: None,
            database: None,
            reset: false,
            given: false,
        }
    }
}

impl Default for PrinterConfig {
    fn default() -> Self {
        Self {
            name: "printer".into(),
            listen: ([127, 0, 0, 1], 9100).into(),
            dialect: Dialect::EscPos,
            profile: Profile::generic(),
            settings: Settings::default(),
        }
    }
}

/// A printer as written in `--printer` or in an API request. Anything left out gets its default.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PrinterSettings {
    /// What the API calls it: letters, digits, `-` and `_`. `printer` when left out.
    #[schema(example = "till")]
    pub name: Option<String>,
    /// The address of its TCP port, such as `127.0.0.1:9100`. Port 0 picks a free one.
    #[schema(example = "127.0.0.1:9100")]
    pub listen: Option<String>,
    /// The command language it reads: `escpos`, `starline` or `starprnt`.
    #[schema(example = "escpos")]
    pub dialect: Option<String>,
    /// The printer model, as `GET /api/profiles` lists them. It decides the code pages, the
    /// columns and the answers.
    #[schema(example = "TM-T88IV")]
    pub profile: Option<String>,
    /// What it is set to. What is left out gets its default.
    #[serde(flatten)]
    pub settings: SettingsChange,
    #[serde(flatten)]
    #[schema(ignore)]
    _unknown: Unknown,
}

impl PrinterSettings {
    /// Reads `key=value` pairs through the same declaration as a request, so a setting that is
    /// added to it can be written on the command line and in a request alike.
    fn from_pairs(text: &str) -> Result<Self, String> {
        let mut pairs = serde_json::Map::new();
        for pair in text.split(',').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("`{pair}` should be key=value"))?;
            pairs.insert(key.into(), serde_json::Value::String(value.into()));
        }
        serde_json::from_value(serde_json::Value::Object(pairs))
            .map_err(|error| format!("unknown or wrong printer setting: {error}"))
    }

    pub(crate) fn build(self) -> Result<PrinterConfig, String> {
        let mut printer = PrinterConfig::default();
        if let Some(name) = self.name {
            printer.name = name;
        }
        if let Some(listen) = self.listen {
            printer.listen = address(&listen)?;
        }
        if let Some(dialect) = self.dialect {
            printer.dialect = Dialect::from_name(&dialect).ok_or_else(|| {
                format!("`{dialect}` is not a dialect, use escpos, starline or starprnt")
            })?;
        }
        if let Some(profile) = self.profile {
            printer.profile = Profile::named(&profile)
                .ok_or_else(|| format!("`{profile}` is not a printer profile"))?;
        }
        printer.settings = self.settings.applied_to(printer.settings);
        Ok(printer)
    }
}

fn address(text: &str) -> Result<SocketAddr, String> {
    text.parse()
        .map_err(|_| format!("`{text}` is not an address like 127.0.0.1:9100"))
}

impl PrinterConfig {
    /// Says what is wrong with a printer name, if something is.
    pub fn check_name(name: &str) -> Result<(), String> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "the printer name `{name}` should be letters, digits, - and _"
            ));
        }
        Ok(())
    }

    /// A printer written as `name=front,listen=127.0.0.1:9100,dialect=escpos,profile=TM-T88IV,printer_buffer=4096,network_buffer=8192`.
    /// What is left out is the default of [`PrinterConfig::default`].
    pub fn parse(text: &str) -> Result<Self, String> {
        PrinterSettings::from_pairs(text)?.build()
    }
}

/// Where the database is unless told otherwise: `stubprint.db` in the state folder of the system,
/// or nothing when that folder cannot be found.
pub fn default_database() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let folder = if cfg!(windows) {
        var("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library/Application Support")
    } else {
        var("XDG_STATE_HOME").or_else(|| var("HOME").map(|home| home.join(".local/state")))?
    };
    Some(folder.join("stubprint").join("stubprint.db"))
}

impl Config {
    /// Builds the configuration of a command line: the printers given, or the default one.
    pub fn from_options(
        host: Option<IpAddr>,
        port: Option<u16>,
        ready: Option<PathBuf>,
        printers: &[String],
    ) -> Result<Self, String> {
        let given = !printers.is_empty();
        let config = Config {
            host,
            port,
            ready,
            given,
            printers: if given {
                printers
                    .iter()
                    .map(|text| PrinterConfig::parse(text))
                    .collect::<Result<_, _>>()?
            } else {
                Config::default().printers
            },
            ..Config::default()
        };
        config.check()?;
        Ok(config)
    }

    /// Says what is wrong with the configuration, if something is.
    pub fn check(&self) -> Result<(), String> {
        if self.printers.is_empty() {
            return Err("there must be at least one printer".into());
        }
        for (index, printer) in self.printers.iter().enumerate() {
            let name = &printer.name;
            PrinterConfig::check_name(name)?;
            for other in &self.printers[..index] {
                if other.name == *name {
                    return Err(format!("two printers are called `{name}`"));
                }
                if printer.listen.port() != 0 && other.listen == printer.listen {
                    return Err(format!("two printers listen on {}", printer.listen));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_printer_is_a_list_of_settings_and_the_rest_is_the_default() {
        let printer =
            PrinterConfig::parse("name=bar,listen=0.0.0.0:9101,dialect=star-prnt").unwrap();
        assert_eq!(printer.name, "bar");
        assert_eq!(printer.listen, "0.0.0.0:9101".parse().unwrap());
        assert_eq!(printer.dialect, Dialect::StarPrnt);
        assert_eq!(printer.profile, Profile::generic());
    }

    #[test]
    fn the_buffers_of_a_printer_are_written_by_their_names_and_by_the_names_they_had() {
        let printer = PrinterConfig::parse("printer_buffer=1,network_buffer=2").unwrap();
        assert_eq!(
            (
                printer.settings.printer_buffer,
                printer.settings.network_buffer
            ),
            (1, 2)
        );
        let printer = PrinterConfig::parse("hold_offline=3,receive_buffer=4").unwrap();
        assert_eq!(
            (
                printer.settings.printer_buffer,
                printer.settings.network_buffer
            ),
            (3, 4)
        );
        let error = PrinterConfig::parse("printer_buffer=big").unwrap_err();
        assert!(error.contains("whole number"), "{error}");
    }

    #[test]
    fn a_printer_profile_is_found_by_its_name() {
        let printer = PrinterConfig::parse("profile=TM-T88IV").unwrap();
        assert_eq!(printer.profile.name, "TM-T88IV");
    }

    #[test]
    fn what_cannot_be_read_says_which_part() {
        for (text, wanted) in [
            ("dialect=zpl", "dialect"),
            ("listen=nowhere", "address"),
            ("profile=nope", "profile"),
            ("color=red", "color"),
            ("name", "key=value"),
        ] {
            let error = PrinterConfig::parse(text).unwrap_err();
            assert!(error.contains(wanted), "{text}: {error}");
        }
    }

    #[test]
    fn printers_that_are_written_are_given_and_the_default_one_is_not() {
        let written = Config::from_options(None, None, None, &["name=a".to_string()]).unwrap();
        assert!(written.given);
        let default = Config::from_options(None, None, None, &[]).unwrap();
        assert!(!default.given);
        assert_eq!(default.printers.len(), 1);
    }

    #[test]
    fn printers_must_differ_and_names_must_be_safe_in_a_path() {
        let two = |a: &str, b: &str| Config::from_options(None, None, None, &[a.into(), b.into()]);
        assert!(two("name=a,listen=127.0.0.1:1", "name=a,listen=127.0.0.1:2").is_err());
        assert!(two("name=a,listen=127.0.0.1:1", "name=b,listen=127.0.0.1:1").is_err());
        assert!(two("name=a,listen=127.0.0.1:0", "name=b,listen=127.0.0.1:0").is_ok());
        assert!(two("name=a/b", "name=c,listen=127.0.0.1:2").is_err());
    }
}
