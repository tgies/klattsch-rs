//! Reading banks from the canonical JSON schema.
//!
//! The same schema the JS engine and the bundled banks use, so a bank file
//! written for the web engine loads here unchanged. Feature-gated (`json`)
//! because the rest of the crate carries no dependencies.

use serde::Deserialize;

use super::{BankSource, PhonemeEdit, PhonemeEntry, SCHEMA_VERSION};
use crate::phonemes::{GlideTo, PhonemeParams};

/// Why a bank document could not be read.
#[derive(Debug)]
pub enum JsonError {
    /// The document is not valid JSON, or does not match the schema.
    Malformed(serde_json::Error),
    /// The document declares a schema version this crate does not read.
    UnsupportedSchema { bank: String, version: u32 },
}

impl core::fmt::Display for JsonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "malformed bank document: {e}"),
            Self::UnsupportedSchema { bank, version } => write!(
                f,
                "bank `{bank}` declares schemaVersion {version}, expected {SCHEMA_VERSION}"
            ),
        }
    }
}

impl std::error::Error for JsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed(e) => Some(e),
            Self::UnsupportedSchema { .. } => None,
        }
    }
}

impl From<serde_json::Error> for JsonError {
    fn from(e: serde_json::Error) -> Self {
        Self::Malformed(e)
    }
}

/// Parse a bank document.
///
/// Phoneme order follows the document's key order, which is the index space
/// callers address phonemes by, so `serde_json`'s `preserve_order` feature is
/// required (this crate enables it).
///
/// # Errors
///
/// Returns [`JsonError`] for malformed input or an unreadable schema version.
pub fn parse_bank(text: &str) -> Result<BankSource, JsonError> {
    let doc: BankDoc = serde_json::from_str(text)?;
    if doc.schema_version != SCHEMA_VERSION {
        return Err(JsonError::UnsupportedSchema {
            bank: doc.name,
            version: doc.schema_version,
        });
    }
    let mut edits = Vec::with_capacity(doc.phonemes.len());
    for (name, value) in doc.phonemes {
        // A null value deletes an inherited phoneme, matching the JS
        // resolver. No bundled bank uses it, but the schema allows it, so
        // the values are converted one at a time to keep null distinct from
        // a malformed entry.
        if value.is_null() {
            edits.push(PhonemeEdit::Remove(name));
            continue;
        }
        let p: EntryDoc = serde_json::from_value(value)?;
        edits.push(PhonemeEdit::Set(PhonemeEntry {
            params: PhonemeParams {
                f1: p.f1,
                f2: p.f2,
                f3: p.f3,
                bw1: p.bw1,
                bw2: p.bw2,
                bw3: p.bw3,
                a1: p.a1,
                a2: p.a2,
                a3: p.a3,
                voicing: p.voicing,
                glide_to: p.glide_to.map(|g| GlideTo {
                    f1: g.f1,
                    f2: g.f2,
                    f3: g.f3,
                }),
                is_stop: p.is_stop,
            },
            name,
            ipa: p.ipa,
            example: p.example,
        }));
    }

    Ok(BankSource {
        display_name: doc.display_name.unwrap_or_else(|| doc.name.clone()),
        name: doc.name,
        language: doc.language,
        license: doc.license,
        source: doc.source,
        extends: doc.extends,
        edits,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BankDoc {
    schema_version: u32,
    name: String,
    display_name: Option<String>,
    language: Option<String>,
    license: Option<String>,
    source: Option<String>,
    extends: Option<String>,
    /// Raw values, so a `null` (delete an inherited phoneme) stays
    /// distinguishable from a malformed entry. Insertion order is preserved
    /// via `serde_json/preserve_order`, which this crate requires: phoneme
    /// order is the index space callers address phonemes by.
    #[serde(default)]
    phonemes: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct EntryDoc {
    voicing: f32,
    #[serde(rename = "F1")]
    f1: f32,
    #[serde(rename = "F2")]
    f2: f32,
    #[serde(rename = "F3")]
    f3: f32,
    #[serde(rename = "BW1")]
    bw1: f32,
    #[serde(rename = "BW2")]
    bw2: f32,
    #[serde(rename = "BW3")]
    bw3: f32,
    #[serde(rename = "A1")]
    a1: f32,
    #[serde(rename = "A2")]
    a2: f32,
    #[serde(rename = "A3")]
    a3: f32,
    #[serde(rename = "glideTo")]
    glide_to: Option<GlideDoc>,
    #[serde(rename = "isStop", default)]
    is_stop: bool,
    ipa: Option<String>,
    example: Option<String>,
}

#[derive(Deserialize)]
struct GlideDoc {
    #[serde(rename = "F1")]
    f1: f32,
    #[serde(rename = "F2")]
    f2: f32,
    #[serde(rename = "F3")]
    f3: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::banks::BankRegistry;
    use crate::phonemes::PhonemeTable;

    const BASE: &str = r#"{
        "schemaVersion": 1,
        "name": "test-base",
        "displayName": "Test Base",
        "language": "en-US",
        "phonemes": {
            "AA": { "voicing": 1, "F1": 700, "F2": 1220, "F3": 2600,
                    "BW1": 130, "BW2": 70, "BW3": 160,
                    "A1": 1, "A2": 0.9, "A3": 0.7, "ipa": "ɑ", "example": "father" },
            "P":  { "voicing": 0, "F1": 400, "F2": 1100, "F3": 2150,
                    "BW1": 300, "BW2": 150, "BW3": 220,
                    "A1": 0.1, "A2": 0.2, "A3": 0.25, "isStop": true },
            "AY": { "voicing": 1, "F1": 660, "F2": 1200, "F3": 2550,
                    "BW1": 100, "BW2": 70, "BW3": 200,
                    "A1": 1, "A2": 0.9, "A3": 0.7,
                    "glideTo": { "F1": 400, "F2": 1880, "F3": 2500 } }
        }
    }"#;

    #[test]
    fn parses_every_documented_field() {
        let src = parse_bank(BASE).unwrap();
        assert_eq!(src.name, "test-base");
        assert_eq!(src.display_name, "Test Base");
        assert_eq!(src.language.as_deref(), Some("en-US"));
        assert_eq!(src.extends, None);
        assert_eq!(src.edits.len(), 3);

        let mut reg = BankRegistry::new();
        reg.register(src).unwrap();
        let bank = reg.get("test-base").unwrap();

        assert_eq!(bank.name_list(), ["AA", "P", "AY"], "key order preserved");
        let aa = bank.get("AA").unwrap();
        assert_eq!(aa.params.f1, 700.0);
        assert_eq!(aa.params.bw2, 70.0);
        assert_eq!(aa.ipa.as_deref(), Some("ɑ"));
        assert_eq!(aa.example.as_deref(), Some("father"));
        assert!(!aa.params.is_stop);
        assert!(bank.get("P").unwrap().params.is_stop);
        let glide = bank.get("AY").unwrap().params.glide_to.unwrap();
        assert_eq!((glide.f1, glide.f2, glide.f3), (400.0, 1880.0, 2500.0));
    }

    #[test]
    fn null_entry_becomes_a_removal() {
        let child = r#"{
            "schemaVersion": 1, "name": "test-child", "extends": "test-base",
            "phonemes": { "P": null }
        }"#;
        let mut reg = BankRegistry::new();
        reg.register(parse_bank(BASE).unwrap()).unwrap();
        reg.register(parse_bank(child).unwrap()).unwrap();
        assert_eq!(reg.get("test-child").unwrap().name_list(), ["AA", "AY"]);
    }

    #[test]
    fn missing_display_name_falls_back_to_the_bank_name() {
        let doc = r#"{ "schemaVersion": 1, "name": "bare", "phonemes": {} }"#;
        let src = parse_bank(doc).unwrap();
        assert_eq!(src.display_name, "bare");
        assert!(src.edits.is_empty());
    }

    #[test]
    fn wrong_schema_version_is_rejected() {
        let doc = r#"{ "schemaVersion": 2, "name": "future", "phonemes": {} }"#;
        assert!(matches!(
            parse_bank(doc),
            Err(JsonError::UnsupportedSchema { version: 2, .. })
        ));
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(matches!(
            parse_bank("not json"),
            Err(JsonError::Malformed(_))
        ));
        // A phoneme missing a required field is malformed, not silently zeroed.
        let doc = r#"{ "schemaVersion": 1, "name": "x",
                       "phonemes": { "AA": { "voicing": 1 } } }"#;
        assert!(matches!(parse_bank(doc), Err(JsonError::Malformed(_))));
    }
}
