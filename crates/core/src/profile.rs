use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

/// What the printers in `profiles/profiles.json` say about themselves.
const PROFILES_JSON: &str = include_str!("../../../profiles/profiles.json");

/// The links to manuals that the files of the models give, by model.
const MANUALS_JSON: &str = include_str!("../../../profiles/manuals.json");

/// How two byte characters are read once `FS &` has turned them on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanjiEncoding {
    #[default]
    ShiftJis,
    /// The JIS code of a Japanese model, in which both bytes of a character are 0x21 to 0x7e. It
    /// is what `FS C` 0 selects, so a profile never has it, and only a Japanese model has it.
    Jis,
    Gb18030,
    Big5,
    EucKr,
}

/// What a printer answers when it is asked who it is, with `GS I`. The values are only as real as
/// the profile: a model id such as `0x20` is not looked up for each printer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// How wide the paper prints, in millimetres, when the model says.
    pub width_mm: Option<f64>,
    /// How many dots make an inch, when the model says.
    pub dpi: Option<u32>,
    /// Links to the manuals of the model.
    pub manuals: Vec<String>,
    /// How two byte characters are read.
    pub kanji: KanjiEncoding,
    pub identity: Identity,
    code_pages: BTreeMap<u8, String>,
    features: BTreeMap<String, bool>,
}

fn manuals() -> &'static Value {
    static MANUALS: OnceLock<Value> = OnceLock::new();
    MANUALS.get_or_init(|| {
        serde_json::from_str(MANUALS_JSON).expect("profiles/manuals.json is valid JSON")
    })
}

fn database() -> &'static Value {
    static DATABASE: OnceLock<Value> = OnceLock::new();
    DATABASE.get_or_init(|| {
        serde_json::from_str(PROFILES_JSON).expect("profiles/profiles.json is valid JSON")
    })
}

/// The upper half of a code page that the database gives as a table, by its name such as `CP3012`.
/// A byte from 0x80 up is the character at that place. A gap in the table is a space.
pub(crate) fn char_table(name: &str) -> Option<&'static [char; 128]> {
    static TABLES: OnceLock<BTreeMap<String, [char; 128]>> = OnceLock::new();
    TABLES
        .get_or_init(|| {
            let mut tables = BTreeMap::new();
            for (name, item) in database()["encodings"].as_object().into_iter().flatten() {
                let rows = item["data"].as_array().into_iter().flatten();
                let chars: Vec<char> = rows
                    .flat_map(|row| row.as_str().unwrap_or("").chars().collect::<Vec<_>>())
                    .collect();
                if let Ok(table) = <[char; 128]>::try_from(chars) {
                    tables.insert(name.to_ascii_uppercase(), table);
                }
            }
            tables
        })
        .get(&name.to_ascii_uppercase())
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

        // Fonts are numbered from 0, font A, to 4, font E. A model may skip one, which is left at 0
        // columns, and may have special fonts with higher numbers, which `ESC M` cannot choose here.
        let mut columns = Vec::new();
        if let Some(fonts) = data["fonts"].as_object() {
            for (number, font) in fonts {
                let Ok(number) = number.parse::<usize>() else {
                    continue;
                };
                if number > 4 {
                    continue;
                }
                columns.resize(columns.len().max(number + 1), 0);
                columns[number] = font["columns"].as_u64().unwrap_or(0) as u8;
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
            width_mm: data["media"]["width"]["mm"].as_f64(),
            dpi: data["media"]["dpi"].as_u64().map(|dpi| dpi as u32),
            manuals: manuals()[key.as_str()]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|link| link.as_str().map(str::to_owned))
                .collect(),
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

    /// How many code pages `ESC t` can choose on this model.
    pub fn code_page_count(&self) -> usize {
        self.code_pages.len()
    }

    /// The features the model has, such as `qrCode`, in alphabetical order.
    pub fn features(&self) -> Vec<&str> {
        self.features
            .iter()
            .filter(|(_, on)| **on)
            .map(|(name, _)| name.as_str())
            .collect()
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
    fn the_database_has_the_printers_the_rest_depends_on() {
        for name in [
            "default", "simple", "TM-T88IV", "TM-T88V", "TM-P80", "SP2000",
        ] {
            assert!(Profile::named(name).is_some(), "{name} is missing");
        }
        assert!(Profile::named("tm-t88iv").is_some());
        assert!(Profile::named("no such printer").is_none());
    }

    #[test]
    fn every_profile_loads_and_its_widths_make_sense() {
        let names = Profile::names();
        let mut lower: Vec<String> = names.iter().map(|name| name.to_lowercase()).collect();
        lower.sort();
        lower.dedup();
        assert_eq!(lower.len(), names.len(), "two names differ only in case");
        for name in names {
            let profile = Profile::named(&name).unwrap_or_else(|| panic!("{name} does not load"));
            assert!(
                profile.columns.first().is_none_or(|columns| *columns > 0),
                "{name}: font A has no columns"
            );
            // A character is at least 5 dots wide, which is the smallest font there is.
            if let (Some(dots), Some(columns)) = (profile.width_dots, profile.columns.first()) {
                assert!(
                    dots / u32::from(*columns) >= 5,
                    "{name}: {columns} columns do not fit in {dots} dots"
                );
            }
        }
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
