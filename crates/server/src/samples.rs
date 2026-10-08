//! The samples that `samples/python/build.py` wrote, embedded by `build.rs`.

use std::sync::LazyLock;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use stubprint_core::Dialect;

const DATA: &str = include_str!(concat!(env!("OUT_DIR"), "/samples.json"));

/// A receipt and the variants that libraries wrote of it.
#[derive(Debug)]
pub struct Sample {
    pub id: String,
    pub title: String,
    pub about: String,
    /// The printer model it was written for, as `GET /api/profiles` lists them.
    pub profile: String,
    pub tags: Vec<String>,
    /// What a decoder must find on the paper.
    pub expect: Value,
    /// The receipt as data.
    pub receipt: Value,
    pub variants: Vec<Variant>,
}

/// The bytes one library wrote.
#[derive(Debug)]
pub struct Variant {
    pub library: String,
    pub version: String,
    pub language: String,
    pub dialect: Dialect,
    pub bytes: Vec<u8>,
}

#[derive(Deserialize)]
struct StoredSample {
    id: String,
    title: String,
    about: String,
    profile: String,
    tags: Vec<String>,
    expect: Value,
    receipt: Value,
    variants: Vec<StoredVariant>,
}

#[derive(Deserialize)]
struct StoredVariant {
    library: String,
    version: String,
    language: String,
    dialect: String,
    bytes: String,
}

impl StoredSample {
    fn read(self) -> Result<Sample, String> {
        let variants = self
            .variants
            .into_iter()
            .map(|variant| {
                Ok(Variant {
                    dialect: Dialect::from_name(&variant.dialect).ok_or_else(|| {
                        format!("sample {}: no dialect {}", self.id, variant.dialect)
                    })?,
                    bytes: STANDARD
                        .decode(&variant.bytes)
                        .map_err(|error| format!("sample {}: {error}", self.id))?,
                    library: variant.library,
                    version: variant.version,
                    language: variant.language,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Sample {
            id: self.id,
            title: self.title,
            about: self.about,
            profile: self.profile,
            tags: self.tags,
            expect: self.expect,
            receipt: self.receipt,
            variants,
        })
    }
}

static ALL: LazyLock<Vec<Sample>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<StoredSample>>(DATA)
        .map_err(|error| error.to_string())
        .and_then(|stored| stored.into_iter().map(StoredSample::read).collect())
        .expect("samples/out/samples.json is made by samples/python/build.py and can be read")
});

/// Every sample, in display order.
pub fn all() -> &'static [Sample] {
    &ALL
}

/// The sample with this id.
pub fn find(id: &str) -> Option<&'static Sample> {
    ALL.iter().find(|sample| sample.id == id)
}

impl Sample {
    /// The first variant matching the library and dialect that are given.
    pub fn variant(&self, library: Option<&str>, dialect: Option<Dialect>) -> Option<&Variant> {
        self.variants.iter().find(|variant| {
            library.is_none_or(|library| variant.library == library)
                && dialect.is_none_or(|dialect| variant.dialect == dialect)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ids_fit_in_an_address_and_are_all_different() {
        let mut ids: Vec<_> = all().iter().map(|sample| sample.id.as_str()).collect();
        assert!(ids.iter().all(|id| id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
    }

    #[test]
    fn every_sample_is_written_for_a_printer_model_that_exists() {
        for sample in all() {
            assert!(
                stubprint_core::Profile::named(&sample.profile).is_some(),
                "{}: no profile called {}",
                sample.id,
                sample.profile
            );
        }
    }

    #[test]
    fn every_sample_has_a_variant_with_bytes() {
        for sample in all() {
            assert!(!sample.variants.is_empty(), "{}", sample.id);
            for variant in &sample.variants {
                assert!(!variant.bytes.is_empty(), "{}", sample.id);
            }
        }
    }
}
