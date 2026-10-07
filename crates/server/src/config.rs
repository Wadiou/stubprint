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
    /// A folder with the built web page, which the program then serves next to the API.
    pub web: Option<PathBuf>,
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http: ([127, 0, 0, 1], 8080).into(),
            printers: vec![PrinterConfig::default()],
            keep: 500,
            web: None,
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
        }
    }
}

/// How a printer is written in a configuration file, and how a `--printer` option is written once
/// it is read as `key=value` pairs.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrinterSettings {
    name: Option<String>,
    listen: Option<String>,
    dialect: Option<String>,
    profile: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSettings {
    http: Option<String>,
    keep: Option<usize>,
    web: Option<PathBuf>,
    printers: Option<Vec<PrinterSettings>>,
}

impl PrinterSettings {
    fn from_pairs(text: &str) -> Result<Self, String> {
        let mut settings = PrinterSettings {
            name: None,
            listen: None,
            dialect: None,
            profile: None,
        };
        for pair in text.split(',').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("`{pair}` should be key=value"))?;
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

    fn build(self) -> Result<PrinterConfig, String> {
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
        Ok(printer)
    }
}

fn address(text: &str) -> Result<SocketAddr, String> {
    text.parse()
        .map_err(|_| format!("`{text}` is not an address like 127.0.0.1:9100"))
}

impl PrinterConfig {
    /// A printer written as `name=front,listen=127.0.0.1:9100,dialect=escpos,profile=TM-T88IV`.
    /// What is left out is the default of [`PrinterConfig::default`].
    pub fn parse(text: &str) -> Result<Self, String> {
        PrinterSettings::from_pairs(text)?.build()
    }
}

impl Config {
    /// A configuration written as JSON, with `http`, `keep`, `web` and `printers` that each have `name`,
    /// `listen`, `dialect` and `profile`. What is left out is the default.
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
        config.web = file.web;
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
        printers: &[String],
    ) -> Result<Self, String> {
        let mut config = Config::default();
        if let Some(http) = http {
            config.http = http;
        }
        if let Some(keep) = keep {
            config.keep = keep;
        }
        config.web = web;
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
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(format!(
                    "the printer name `{name}` should be letters, digits, - and _"
                ));
            }
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
        let two = |a: &str, b: &str| Config::from_options(None, None, None, &[a.into(), b.into()]);
        assert!(two("name=a,listen=127.0.0.1:1", "name=a,listen=127.0.0.1:2").is_err());
        assert!(two("name=a,listen=127.0.0.1:1", "name=b,listen=127.0.0.1:1").is_err());
        assert!(two("name=a,listen=127.0.0.1:0", "name=b,listen=127.0.0.1:0").is_ok());
        assert!(two("name=a/b", "name=c,listen=127.0.0.1:2").is_err());
        assert!(Config::from_options(None, Some(0), None, &[]).is_err());
    }
}
