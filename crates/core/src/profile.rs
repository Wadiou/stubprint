use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

/// What the printers in `profiles/profiles.json` say about themselves.
const PROFILES_JSON: &str = include_str!("../profiles/profiles.json");

/// How two byte characters are read once `FS &` has turned them on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanjiEncoding {
    #[default]
    ShiftJis,
    Gb18030,
    Big5,
    EucKr,
}

/// What a printer answers when it is asked who it is, with `GS I`. The values are only as real as
/// the profile: a model id such as `0x20` is not looked up for each printer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub serial: String,
    /// The answer to `GS I 1`.
    pub model_id: u8,
    /// The answer to `GS I 2`. Bit 1 says there is an auto cutter.
    pub type_id: u8,
}

/// A printer model: which code page each `ESC t` number picks, how wide its fonts are, how wide the
/// paper is and which commands it has. The data comes from a database that people keep for
/// receipt printers, and the numbers differ a lot between makers.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    pub vendor: String,
    /// How many columns each font has, font A first.
    pub columns: Vec<u8>,
    /// The width of the paper in dots, when the model says.
    pub width_dots: Option<u32>,
    /// How two byte characters are read.
    pub kanji: KanjiEncoding,
    pub identity: Identity,
    code_pages: BTreeMap<u8, String>,
    features: BTreeMap<String, bool>,
}

fn database() -> &'static Value {
    static DATABASE: OnceLock<Value> = OnceLock::new();
    DATABASE.get_or_init(|| {
        serde_json::from_str(PROFILES_JSON).expect("profiles/profiles.json is valid JSON")
    })
}

impl Profile {
    /// The profile every printer without a better one gets: the usual Epson numbers.
    pub fn generic() -> Self {
        Self::named("default").expect("the database has a default profile")
    }

    /// The names of all the profiles, such as `TM-T88IV` or `SP2000`.
    pub fn names() -> Vec<String> {
        database()["profiles"]
            .as_object()
            .map(|profiles| profiles.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// A profile by name. The case does not matter.
    pub fn named(name: &str) -> Option<Self> {
        let profiles = database()["profiles"].as_object()?;
        let (key, data) = profiles
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))?;

        let code_pages = data["codePages"]
            .as_object()
            .map(|pages| {
                pages
                    .iter()
                    .filter_map(|(number, name)| {
                        let name = name.as_str().filter(|name| *name != "Unknown")?;
                        Some((number.parse::<u8>().ok()?, name.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut columns = Vec::new();
        if let Some(fonts) = data["fonts"].as_object() {
            for index in 0..fonts.len() {
                columns.push(fonts[&index.to_string()]["columns"].as_u64().unwrap_or(0) as u8);
            }
        }

        let features = data["features"]
            .as_object()
            .map(|features| {
                features
                    .iter()
                    .map(|(name, on)| (name.clone(), on.as_bool().unwrap_or(false)))
                    .collect()
            })
            .unwrap_or_default();

        let vendor = data["vendor"].as_str().unwrap_or("").to_owned();
        let model = data["name"].as_str().unwrap_or(key).to_owned();
        Some(Self {
            name: key.clone(),
            columns,
            width_dots: data["media"]["width"]["pixels"]
                .as_u64()
                .map(|dots| dots as u32),
            kanji: KanjiEncoding::default(),
            identity: Identity {
                manufacturer: if vendor.is_empty() || vendor == "Generic" {
                    "EPSON".to_owned()
                } else {
                    vendor.to_uppercase()
                },
                model,
                firmware: "1.00 ESC/POS".to_owned(),
                serial: "000000000".to_owned(),
                model_id: 0x20,
                type_id: 0x02,
            },
            vendor,
            code_pages,
            features,
        })
    }

    /// The name of the code page that `ESC t number` selects, such as `CP437`, if the printer has
    /// one there.
    pub fn code_page(&self, number: u8) -> Option<&str> {
        self.code_pages.get(&number).map(String::as_str)
    }

    /// Whether the printer has a feature, such as `qrCode`, `pdf417Code` or `starCommands`.
    pub fn supports(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(false)
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::generic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_database_has_every_printer() {
        assert_eq!(Profile::names().len(), 24);
        assert!(Profile::named("tm-t88iv").is_some());
        assert!(Profile::named("no such printer").is_none());
    }

    #[test]
    fn the_same_number_is_another_code_page_on_another_printer() {
        let epson = Profile::named("TM-T88IV").unwrap();
        let star = Profile::named("SP2000").unwrap();
        assert_eq!(epson.code_page(2), Some("CP850"));
        assert_eq!(star.code_page(2), Some("CP932"));
        assert_eq!(epson.code_page(50), None);
        assert_eq!(Profile::generic().code_page(50), Some("CP1256"));
    }

    #[test]
    fn a_profile_says_how_wide_the_paper_and_fonts_are() {
        let profile = Profile::named("TM-T88IV").unwrap();
        assert_eq!(profile.width_dots, Some(512));
        assert_eq!(profile.columns, vec![42, 56]);
        assert!(profile.supports("qrCode"));
        assert!(!profile.supports("starCommands"));
        assert!(Profile::named("SP2000").unwrap().supports("starCommands"));
    }
}
