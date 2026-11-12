use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;
use stubprint_core::{Dialect, Profile};

/// What the server does: where the REST API is, and which virtual printers listen.
#[derive(Debug, Clone)]
pub struct Config {
    /// Where the REST API and the live events are served.
    pub http: SocketAddr,
    pub printers: Vec<PrinterConfig>,
    /// How many receipts are kept. The oldest are forgotten first.
    pub keep: usize,
    /// The web page served next to the API.
    pub web: Web,
    /// A file to write the addresses into once everything listens, so a program that started the
    /// server on port 0 can find it. It is removed when the server stops.
    pub ready: Option<PathBuf>,
    /// A file to keep the printers in between runs, each with its settings and what it reports.
    pub state: Option<PathBuf>,
    /// Start the printers of `state` when it exists, instead of `printers`. It is false when the
    /// printers were given, since what is asked for now wins over what was kept.
    pub restore: bool,
}

/// Where the web page served next to the API comes from.
#[derive(Debug, Clone, Default)]
pub enum Web {
    /// The page that is in the program, when it was built with one.
    #[default]
    Embedded,
    /// A folder with a built page, such as one that is being worked on.
    Folder(PathBuf),
    /// No page, only the API.
    Off,
}

impl Web {
    /// Whether a page is served at all.
    pub fn is_served(&self) -> bool {
        match self {
            Web::Embedded => crate::page::is_built(),
            Web::Folder(_) => true,
            Web::Off => false,
        }
    }
}

/// One virtual printer. It listens on its own port, which is what sets its command language, like
/// the emulation setting of a real printer.
#[derive(Debug, Clone)]
pub struct PrinterConfig {
    /// What the API calls it. Letters, digits, `-` and `_`.
    pub name: String,
    pub listen: SocketAddr,
    pub dialect: Dialect,
    pub profile: Profile,
    /// How many bytes it holds while it cannot print, such as with its cover open. 0 means it holds
    /// nothing and prints whatever it receives, whatever it reports.
    pub hold_offline: usize,
}

/// The size of the receive buffer of a printer, in bytes, when none is chosen.
pub const DEFAULT_HOLD: usize = 4096;

impl Default for Config {
    fn default() -> Self {
        Self {
            http: ([127, 0, 0, 1], 8080).into(),
            printers: vec![PrinterConfig::default()],
            keep: 500,
            web: Web::default(),
            ready: None,
            state: None,
            restore: false,
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
            hold_offline: DEFAULT_HOLD,
        }
    }
}

/// A printer as it is written in a configuration file or in a request, where what is left out is
/// the default.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
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
    /// How many bytes it holds while it cannot print, as a real printer keeps them in its receive
    /// buffer: it is off line, its cover is open, its paper has run out or it has a fault. A program
    /// that sends more has to wait. What it held prints when the printer can print again, even if
    /// the program has gone. The default is 4096. 0 holds nothing: it prints whatever it receives.
    #[schema(example = 4096)]
    pub hold_offline: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSettings {
    http: Option<String>,
    keep: Option<usize>,
    web: Option<PathBuf>,
    ready_file: Option<PathBuf>,
    state_file: Option<PathBuf>,
    printers: Option<Vec<PrinterSettings>>,
}

impl PrinterSettings {
    fn from_pairs(text: &str) -> Result<Self, String> {
        let mut settings = PrinterSettings {
            name: None,
            listen: None,
            dialect: None,
            profile: None,
            hold_offline: None,
        };
        for pair in text.split(',').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("`{pair}` should be key=value"))?;
            if key == "hold_offline" {
                settings.hold_offline = Some(
                    value
                        .parse()
                        .map_err(|_| format!("`{value}` is not a number of bytes"))?,
                );
                continue;
            }
            let slot = match key {
                "name" => &mut settings.name,
                "listen" => &mut settings.listen,
                "dialect" => &mut settings.dialect,
                "profile" => &mut settings.profile,
                _ => return Err(format!("unknown printer setting `{key}`")),
            };
            *slot = Some(value.to_string());
        }
        Ok(settings)
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
        if let Some(bytes) = self.hold_offline {
            printer.hold_offline = bytes;
        }
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

    /// A printer written as `name=front,listen=127.0.0.1:9100,dialect=escpos,profile=TM-T88IV,hold_offline=4096`.
    /// What is left out is the default of [`PrinterConfig::default`].
    pub fn parse(text: &str) -> Result<Self, String> {
        PrinterSettings::from_pairs(text)?.build()
    }
}

/// Where the printers are kept between runs unless told otherwise: the state folder of the system,
/// or nothing when it cannot be found.
pub fn default_state_file() -> Option<PathBuf> {
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
    Some(folder.join("stubprint").join("state.json"))
}

impl Config {
    /// A configuration written as JSON, with `http`, `keep`, `web`, `ready_file`, `state_file` and `printers` that each have `name`,
    /// `listen`, `dialect`, `profile` and `hold_offline`. What is left out is the default.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: FileSettings =
            serde_json::from_str(text).map_err(|error| format!("bad configuration: {error}"))?;
        let mut config = Config::default();
        if let Some(http) = file.http {
            config.http = address(&http)?;
        }
        if let Some(keep) = file.keep {
            config.keep = keep;
        }
        if let Some(folder) = file.web {
            config.web = Web::Folder(folder);
        }
        config.ready = file.ready_file;
        config.state = file.state_file;
        config.restore = file.printers.is_none();
        if let Some(printers) = file.printers {
            config.printers = printers
                .into_iter()
                .map(PrinterSettings::build)
                .collect::<Result<_, _>>()?;
        }
        config.check()?;
        Ok(config)
    }

    /// Builds the configuration of a command line: the printers given, or the default one.
    pub fn from_options(
        http: Option<SocketAddr>,
        keep: Option<usize>,
        web: Option<PathBuf>,
        ready: Option<PathBuf>,
        printers: &[String],
    ) -> Result<Self, String> {
        let mut config = Config::default();
        if let Some(http) = http {
            config.http = http;
        }
        if let Some(keep) = keep {
            config.keep = keep;
        }
        if let Some(folder) = web {
            config.web = Web::Folder(folder);
        }
        config.ready = ready;
        config.restore = printers.is_empty();
        if !printers.is_empty() {
            config.printers = printers
                .iter()
                .map(|text| PrinterConfig::parse(text))
                .collect::<Result<_, _>>()?;
        }
        config.check()?;
        Ok(config)
    }

    /// Says what is wrong with the configuration, if something is.
    pub fn check(&self) -> Result<(), String> {
        if self.keep == 0 {
            return Err("keep must be at least 1".into());
        }
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
    fn a_configuration_file_has_the_same_settings() {
        let config = Config::from_json(
            r#"{"http":"0.0.0.0:80","keep":3,"printers":[
                {"name":"a","listen":"127.0.0.1:1","dialect":"escpos"},
                {"name":"b","listen":"127.0.0.1:2","dialect":"starline"}]}"#,
        )
        .unwrap();
        assert_eq!(config.keep, 3);
        assert_eq!(config.printers[1].dialect, Dialect::StarLine);
        assert!(Config::from_json(r#"{"colour":1}"#).is_err());
    }

    #[test]
    fn printers_must_differ_and_names_must_be_safe_in_a_path() {
        let two =
            |a: &str, b: &str| Config::from_options(None, None, None, None, &[a.into(), b.into()]);
        assert!(two("name=a,listen=127.0.0.1:1", "name=a,listen=127.0.0.1:2").is_err());
        assert!(two("name=a,listen=127.0.0.1:1", "name=b,listen=127.0.0.1:1").is_err());
        assert!(two("name=a,listen=127.0.0.1:0", "name=b,listen=127.0.0.1:0").is_ok());
        assert!(two("name=a/b", "name=c,listen=127.0.0.1:2").is_err());
        assert!(Config::from_options(None, Some(0), None, None, &[]).is_err());
    }
}
