//! Phoneme banks: named tables of phoneme parameters that can extend one
//! another.
//!
//! A bank is authored as a [`BankSource`] (metadata, an optional `extends`
//! parent, and an ordered list of edits) and resolved into a [`Bank`], which
//! implements [`PhonemeTable`]. Resolution flattens the `extends` chain the
//! way the reference JS implementation does: the parent's entries first
//! (recursively resolved), then this bank's edits overlaid, where a same-name
//! entry **overrides in place** and a new one appends. Entry order is
//! load-bearing: it determines phoneme indices for callers that address
//! phonemes numerically.
//!
//! Banks are data, not code. The bundled banks in [`bundled`] are generated
//! from the canonical JSON in the JS engine repo by `tools/gen-banks.mjs`;
//! runtime banks come from the same schema through the `json` feature.

use alloc_shim::{String, ToOwned, Vec};

use crate::phonemes::{GlideTo, PhonemeParams, PhonemeTable};

pub mod bundled;

#[cfg(feature = "json")]
mod json;

#[cfg(feature = "json")]
pub use json::{parse_bank, JsonError};

/// Re-exports so this module reads the same with or without `no_std` work
/// later; today it is a plain alias for the std types.
mod alloc_shim {
    pub use std::borrow::ToOwned;
    pub use std::string::String;
    pub use std::vec::Vec;
}

/// The bank name used when a caller does not choose one.
pub const DEFAULT_BANK: &str = "klatt1980-en";

/// The only schema version this crate reads.
pub const SCHEMA_VERSION: u32 = 1;

/// One phoneme in a bank: its parameters plus the display metadata the JSON
/// carries for editor legends.
#[derive(Clone, Debug, PartialEq)]
pub struct PhonemeEntry {
    pub name: String,
    pub params: PhonemeParams,
    /// IPA symbol, for editor legends.
    pub ipa: Option<String>,
    /// Example word, for editor legends.
    pub example: Option<String>,
}

impl PhonemeEntry {
    /// A minimal entry with no display metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, params: PhonemeParams) -> Self {
        Self {
            name: name.into(),
            params,
            ipa: None,
            example: None,
        }
    }
}

/// One authored change to the inherited phoneme set.
#[derive(Clone, Debug, PartialEq)]
pub enum PhonemeEdit {
    /// Add the phoneme, or replace an inherited one of the same name in place.
    Set(PhonemeEntry),
    /// Drop an inherited phoneme. The JSON spells this as a `null` value.
    Remove(String),
}

impl PhonemeEdit {
    fn name(&self) -> &str {
        match self {
            Self::Set(entry) => &entry.name,
            Self::Remove(name) => name,
        }
    }
}

/// An authored bank, before its `extends` chain is flattened.
#[derive(Clone, Debug, PartialEq)]
pub struct BankSource {
    pub name: String,
    pub display_name: String,
    pub language: Option<String>,
    pub license: Option<String>,
    pub source: Option<String>,
    /// Name of the bank this one inherits from, if any.
    pub extends: Option<String>,
    pub edits: Vec<PhonemeEdit>,
}

impl BankSource {
    /// A base bank (no parent) whose edits are all additions.
    #[must_use]
    pub fn new(name: impl Into<String>, entries: Vec<PhonemeEntry>) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            language: None,
            license: None,
            source: None,
            extends: None,
            edits: entries.into_iter().map(PhonemeEdit::Set).collect(),
        }
    }
}

/// A resolved bank: metadata plus the flattened phoneme list, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Bank {
    name: String,
    display_name: String,
    language: Option<String>,
    license: Option<String>,
    source: Option<String>,
    entries: Vec<PhonemeEntry>,
}

impl Bank {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// BCP 47 language tag, when the bank declares one.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }

    /// Provenance note for the phoneme data.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// The phonemes in bank order.
    #[must_use]
    pub fn entries(&self) -> &[PhonemeEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry at a bank index, for callers that address phonemes
    /// numerically (keyswitch maps, tracker effect values).
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&PhonemeEntry> {
        self.entries.get(index)
    }

    /// The bank index of a phoneme name.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    /// The full entry for a phoneme name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PhonemeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

impl PhonemeTable for Bank {
    fn lookup(&self, name: &str) -> Option<PhonemeParams> {
        self.get(name).map(|e| e.params)
    }

    fn name_list(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }
}

/// Why a bank could not be registered or resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BankError {
    /// A bank extends a name the registry does not know.
    UnknownParent { bank: String, parent: String },
    /// An `extends` chain loops back on itself.
    ExtendsCycle { path: Vec<String> },
    /// A bank was registered under a name already in use.
    DuplicateName(String),
    /// The bank declared a schema version this crate does not read.
    UnsupportedSchema { bank: String, version: u32 },
}

impl core::fmt::Display for BankError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownParent { bank, parent } => {
                write!(f, "bank `{bank}` extends unknown bank `{parent}`")
            }
            Self::ExtendsCycle { path } => {
                write!(f, "bank extends cycle: {}", path.join(" -> "))
            }
            Self::DuplicateName(name) => write!(f, "bank `{name}` is already registered"),
            Self::UnsupportedSchema { bank, version } => write!(
                f,
                "bank `{bank}` declares schemaVersion {version}, expected {SCHEMA_VERSION}"
            ),
        }
    }
}

impl std::error::Error for BankError {}

/// Named banks with their `extends` chains resolved.
///
/// Construction and registration allocate; do that off the audio thread.
/// Lookups afterwards only borrow.
#[derive(Clone, Debug)]
pub struct BankRegistry {
    authored: Vec<BankSource>,
    resolved: Vec<Bank>,
    default_name: String,
}

impl BankRegistry {
    /// An empty registry. [`BankRegistry::with_bundled`] is usually what you
    /// want.
    #[must_use]
    pub fn new() -> Self {
        Self {
            authored: Vec::new(),
            resolved: Vec::new(),
            default_name: DEFAULT_BANK.to_owned(),
        }
    }

    /// Every bank shipped with this crate, resolved.
    #[must_use]
    pub fn with_bundled() -> Self {
        let mut registry = Self::new();
        for source in bundled::sources() {
            registry
                .register(source)
                .expect("bundled banks are self-consistent");
        }
        registry
    }

    /// Add a bank and resolve it. Registering a bank whose parent is already
    /// present resolves immediately; registering one whose parent is missing
    /// is an error, so register base banks first.
    ///
    /// # Errors
    ///
    /// Returns [`BankError`] when the name is taken, the parent is unknown, or
    /// the `extends` chain loops.
    pub fn register(&mut self, source: BankSource) -> Result<(), BankError> {
        if self.authored.iter().any(|b| b.name == source.name) {
            return Err(BankError::DuplicateName(source.name));
        }
        let name = source.name.clone();
        self.authored.push(source);
        // Resolve eagerly so a bad chain is reported at registration rather
        // than at first use. On failure the authored entry is rolled back.
        match self.build(&name, &mut Vec::new()) {
            Ok(bank) => {
                self.resolved.push(bank);
                Ok(())
            }
            Err(e) => {
                self.authored.retain(|b| b.name != name);
                Err(e)
            }
        }
    }

    /// Name of the bank [`resolve`][Self::resolve] falls back to.
    #[must_use]
    pub fn default_name(&self) -> &str {
        &self.default_name
    }

    /// Change the fallback bank. The name need not exist yet.
    pub fn set_default_name(&mut self, name: impl Into<String>) {
        self.default_name = name.into();
    }

    /// Registered bank names, in registration order.
    pub fn list(&self) -> impl Iterator<Item = &str> {
        self.authored.iter().map(|b| b.name.as_str())
    }

    /// A resolved bank by name, or `None` when it is not registered.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Bank> {
        self.resolved.iter().find(|b| b.name == name)
    }

    /// A resolved bank by name, falling back to the default bank, then to any
    /// registered bank. `None` only when the registry is empty.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&Bank> {
        self.get(name)
            .or_else(|| self.get(&self.default_name))
            .or_else(|| self.resolved.first())
    }

    fn source(&self, name: &str) -> Option<&BankSource> {
        self.authored.iter().find(|b| b.name == name)
    }

    /// Flatten one bank's `extends` chain. `visiting` carries the chain being
    /// resolved so a cycle is reported rather than recursed into.
    fn build(&self, name: &str, visiting: &mut Vec<String>) -> Result<Bank, BankError> {
        if visiting.iter().any(|v| v == name) {
            let mut path = visiting.clone();
            path.push(name.to_owned());
            return Err(BankError::ExtendsCycle { path });
        }
        let source = self.source(name).ok_or_else(|| BankError::UnknownParent {
            bank: visiting.last().cloned().unwrap_or_default(),
            parent: name.to_owned(),
        })?;

        let mut entries = match &source.extends {
            Some(parent) => {
                visiting.push(name.to_owned());
                let resolved = self.build(parent, visiting);
                visiting.pop();
                resolved?.entries
            }
            None => Vec::new(),
        };

        for edit in &source.edits {
            let existing = entries.iter().position(|e| e.name == edit.name());
            match (edit, existing) {
                // Same-name entries override in place: bank order is an
                // index space, so a redefinition must not renumber phonemes.
                (PhonemeEdit::Set(entry), Some(i)) => entries[i] = entry.clone(),
                (PhonemeEdit::Set(entry), None) => entries.push(entry.clone()),
                (PhonemeEdit::Remove(_), Some(i)) => {
                    entries.remove(i);
                }
                (PhonemeEdit::Remove(_), None) => {}
            }
        }

        Ok(Bank {
            name: source.name.clone(),
            display_name: source.display_name.clone(),
            language: source.language.clone(),
            license: source.license.clone(),
            source: source.source.clone(),
            entries,
        })
    }
}

impl Default for BankRegistry {
    fn default() -> Self {
        Self::with_bundled()
    }
}

/// Build [`PhonemeParams`] from the field order the generated bank data uses.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub(crate) const fn params(
    voicing: f32,
    f1: f32,
    f2: f32,
    f3: f32,
    bw1: f32,
    bw2: f32,
    bw3: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    glide_to: Option<GlideTo>,
    is_stop: bool,
) -> PhonemeParams {
    PhonemeParams {
        f1,
        f2,
        f3,
        bw1,
        bw2,
        bw3,
        a1,
        a2,
        a3,
        voicing,
        glide_to,
        is_stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, f1: f32) -> PhonemeEntry {
        PhonemeEntry::new(
            name,
            params(
                1.0, f1, 1500.0, 2500.0, 80.0, 120.0, 160.0, 1.0, 0.9, 0.7, None, false,
            ),
        )
    }

    fn base() -> BankSource {
        BankSource::new("base", vec![entry("A", 100.0), entry("B", 200.0)])
    }

    fn child(edits: Vec<PhonemeEdit>) -> BankSource {
        BankSource {
            extends: Some("base".to_owned()),
            edits,
            ..BankSource::new("child", Vec::new())
        }
    }

    #[test]
    fn base_bank_keeps_authored_order() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        let bank = reg.get("base").unwrap();
        assert_eq!(bank.name_list(), ["A", "B"]);
        assert_eq!(bank.index_of("B"), Some(1));
        assert_eq!(bank.lookup("A").unwrap().f1, 100.0);
        assert!(bank.lookup("nope").is_none());
    }

    #[test]
    fn override_replaces_in_place_and_additions_append() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        reg.register(child(vec![
            PhonemeEdit::Set(entry("B", 999.0)),
            PhonemeEdit::Set(entry("C", 300.0)),
        ]))
        .unwrap();
        let bank = reg.get("child").unwrap();
        assert_eq!(
            bank.name_list(),
            ["A", "B", "C"],
            "an override keeps the parent's index; only new names append"
        );
        assert_eq!(bank.lookup("B").unwrap().f1, 999.0);
        assert_eq!(bank.lookup("A").unwrap().f1, 100.0, "inherited untouched");
    }

    #[test]
    fn remove_drops_an_inherited_phoneme() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        reg.register(child(vec![PhonemeEdit::Remove("A".to_owned())]))
            .unwrap();
        assert_eq!(reg.get("child").unwrap().name_list(), ["B"]);
        assert_eq!(
            reg.get("base").unwrap().name_list(),
            ["A", "B"],
            "the parent is unaffected"
        );
    }

    #[test]
    fn removing_an_absent_phoneme_is_a_no_op() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        reg.register(child(vec![PhonemeEdit::Remove("ZZ".to_owned())]))
            .unwrap();
        assert_eq!(reg.get("child").unwrap().name_list(), ["A", "B"]);
    }

    #[test]
    fn unknown_parent_is_rejected_and_rolled_back() {
        let mut reg = BankRegistry::new();
        let err = reg
            .register(BankSource {
                extends: Some("missing".to_owned()),
                ..BankSource::new("orphan", vec![entry("A", 1.0)])
            })
            .unwrap_err();
        assert!(matches!(err, BankError::UnknownParent { .. }));
        assert_eq!(
            reg.list().count(),
            0,
            "a failed registration leaves no authored trace"
        );
    }

    #[test]
    fn extends_cycle_is_rejected() {
        let mut reg = BankRegistry::new();
        // Build the cycle by hand: `a` extends `b`, then `b` extends `a`.
        reg.authored.push(BankSource {
            extends: Some("b".to_owned()),
            ..BankSource::new("a", Vec::new())
        });
        let err = reg
            .register(BankSource {
                extends: Some("a".to_owned()),
                ..BankSource::new("b", vec![entry("X", 1.0)])
            })
            .unwrap_err();
        assert!(matches!(err, BankError::ExtendsCycle { .. }), "{err}");
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        assert_eq!(
            reg.register(base()).unwrap_err(),
            BankError::DuplicateName("base".to_owned())
        );
    }

    #[test]
    fn resolve_falls_back_to_the_default_bank() {
        let mut reg = BankRegistry::new();
        reg.register(base()).unwrap();
        reg.set_default_name("base");
        assert_eq!(reg.resolve("base").unwrap().name(), "base");
        assert_eq!(
            reg.resolve("nonexistent").unwrap().name(),
            "base",
            "an unknown name falls back rather than failing"
        );
        assert!(BankRegistry::new().resolve("anything").is_none());
    }
}
