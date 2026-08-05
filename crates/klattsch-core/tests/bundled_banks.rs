//! Contracts for the bundled phoneme banks.
//!
//! `bundled.rs` is generated from the JS engine's JSON, which is the source of
//! truth for phoneme data across every port. These tests pin the properties a
//! regenerated file must keep: exact values sampled from Klatt's tables, the
//! phoneme order that defines the numeric index space, and the resolution
//! semantics the JS and C++ resolvers share.

use klattsch_core::banks::{BankRegistry, DEFAULT_BANK};
use klattsch_core::PhonemeTable;

/// klatt1980-en's order is the index space furnace's `10xx` effect and the
/// plugin's keyswitch map both address. Reordering it silently rewrites saved
/// projects, so the whole list is pinned.
const KLATT1980_EN_ORDER: [&str; 40] = [
    "IY", "IH", "EH", "AE", "AA", "AO", "AH", "UH", "UW", "ER", "AY", "AW", "EY", "OW", "OY", "W",
    "Y", "R", "L", "M", "N", "NG", "F", "TH", "S", "SH", "V", "DH", "Z", "ZH", "HH", "P", "B", "T",
    "D", "K", "G", "CH", "JH", "_",
];

#[test]
fn bundled_registry_lists_every_bank() {
    let reg = BankRegistry::with_bundled();
    let names: Vec<&str> = reg.list().collect();
    assert_eq!(
        names,
        ["klatt1980-en", "ja-mokhtari-2000", "ja-hecko-2026"],
        "registration order: a bank must follow the one it extends"
    );
    assert_eq!(reg.default_name(), DEFAULT_BANK);
}

#[test]
fn english_bank_matches_the_canonical_order_and_values() {
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("klatt1980-en").unwrap();
    assert_eq!(bank.name_list(), KLATT1980_EN_ORDER);
    assert_eq!(bank.display_name(), "English (Klatt 1980)");
    assert_eq!(bank.language(), Some("en-US"));
    assert!(bank.source().is_some(), "provenance survives generation");

    // Values sampled from Klatt 1980 Tables II-III via the canonical JSON.
    let aa = bank.lookup("AA").unwrap();
    assert_eq!((aa.f1, aa.f2, aa.f3), (700.0, 1220.0, 2600.0));
    assert_eq!((aa.bw1, aa.bw2, aa.bw3), (130.0, 70.0, 160.0));
    assert_eq!((aa.a1, aa.a2, aa.a3), (1.0, 0.9, 0.7));
    assert_eq!(aa.voicing, 1.0);
    assert!(!aa.is_stop);
    assert!(aa.glide_to.is_none());

    let iy = bank.lookup("IY").unwrap();
    assert_eq!((iy.f1, iy.f2, iy.f3), (310.0, 2020.0, 2960.0));

    // The silence phoneme is neutral formants at zero amplitude.
    let sil = bank.lookup("_").unwrap();
    assert_eq!((sil.a1, sil.a2, sil.a3), (0.0, 0.0, 0.0));
    assert_eq!(sil.voicing, 0.0);
}

#[test]
fn diphthongs_and_stops_survive_generation() {
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("klatt1980-en").unwrap();

    let glide = bank
        .lookup("AY")
        .unwrap()
        .glide_to
        .expect("AY is a diphthong");
    assert_eq!((glide.f1, glide.f2, glide.f3), (400.0, 1880.0, 2500.0));

    for stop in ["P", "B", "T", "D", "K", "G", "CH", "JH"] {
        assert!(bank.lookup(stop).unwrap().is_stop, "{stop} is a stop");
    }
    for open in ["AA", "IY", "S", "M"] {
        assert!(!bank.lookup(open).unwrap().is_stop, "{open} is not a stop");
    }
    let diphthongs = KLATT1980_EN_ORDER
        .iter()
        .filter(|n| bank.lookup(n).unwrap().glide_to.is_some())
        .count();
    assert_eq!(diphthongs, 13, "the English bank has 13 gliding vowels");
}

#[test]
fn japanese_banks_extend_english_and_append_their_own() {
    let reg = BankRegistry::with_bundled();
    for name in ["ja-mokhtari-2000", "ja-hecko-2026"] {
        let bank = reg.get(name).unwrap();
        let names = bank.name_list();
        assert_eq!(
            names.len(),
            46,
            "{name}: 40 inherited English phonemes plus 6 of its own"
        );
        assert_eq!(
            &names[..40],
            &KLATT1980_EN_ORDER[..],
            "{name}: inherited phonemes keep their English indices"
        );
        assert_eq!(
            &names[40..],
            ["I", "E", "A", "O", "U", "DX"],
            "{name}: its own phonemes append in document order"
        );
        assert_eq!(bank.language(), Some("ja-JP"));
        // Japanese consonants come from the inherited English set.
        assert!(bank.lookup("K").is_some());
    }
}

#[test]
fn the_two_japanese_banks_are_different_tunings() {
    let reg = BankRegistry::with_bundled();
    let mokhtari = reg.get("ja-mokhtari-2000").unwrap();
    let hecko = reg.get("ja-hecko-2026").unwrap();
    assert_ne!(
        mokhtari.lookup("A").unwrap().f1,
        hecko.lookup("A").unwrap().f1,
        "the banks exist because they tune the vowels differently"
    );
    // Both inherit the same English data untouched.
    assert_eq!(mokhtari.lookup("AA"), hecko.lookup("AA"));
    assert_eq!(
        mokhtari.lookup("AA"),
        reg.get("klatt1980-en").unwrap().lookup("AA")
    );
}

#[test]
fn bank_entries_carry_editor_metadata() {
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("ja-hecko-2026").unwrap();
    let a = bank.get("A").unwrap();
    assert!(a.ipa.is_some(), "IPA symbol survives generation");
    assert!(a.example.is_some(), "example word survives generation");
}

#[test]
fn resolve_falls_back_to_english() {
    let reg = BankRegistry::with_bundled();
    assert_eq!(
        reg.resolve("ja-hecko-2026").unwrap().name(),
        "ja-hecko-2026"
    );
    assert_eq!(
        reg.resolve("no-such-bank").unwrap().name(),
        DEFAULT_BANK,
        "an unknown name renders in the default bank rather than failing"
    );
}

#[test]
fn banks_are_usable_as_phoneme_tables() {
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("ja-mokhtari-2000").unwrap();
    let table: &dyn PhonemeTable = bank;
    assert!(table.lookup("A").is_some());
    assert_eq!(table.name_list().len(), 46);
    assert!(
        table.names().is_empty(),
        "runtime banks cannot serve the 'static name list; name_list is the general accessor"
    );
}

/// The generated file must stay in step with the JSON it came from. Skipped
/// when the JS engine repo is not checked out alongside this one.
#[test]
fn generated_data_matches_the_canonical_json() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../klattsch/src/engine/banks");
    if !dir.is_dir() {
        eprintln!("skipping: {} not present", dir.display());
        return;
    }

    let reg = BankRegistry::with_bundled();
    for name in ["klatt1980-en", "ja-mokhtari-2000", "ja-hecko-2026"] {
        let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
        let parsed = klattsch_core::banks::parse_bank(&text).unwrap();

        // Compare against the same bank re-registered from JSON, so the
        // generator and the runtime parser are checked against each other.
        let mut fresh = BankRegistry::new();
        for dep in ["klatt1980-en", name] {
            if fresh.get(dep).is_some() {
                continue;
            }
            let dep_text = std::fs::read_to_string(dir.join(format!("{dep}.json"))).unwrap();
            fresh
                .register(klattsch_core::banks::parse_bank(&dep_text).unwrap())
                .unwrap();
        }

        let generated = reg.get(name).unwrap();
        let from_json = fresh.get(name).unwrap();
        assert_eq!(
            generated.name_list(),
            from_json.name_list(),
            "{name}: generated phoneme order drifted from the JSON"
        );
        for entry in from_json.entries() {
            assert_eq!(
                generated.get(&entry.name),
                Some(entry),
                "{name}/{}: generated data drifted from the JSON",
                entry.name
            );
        }
        assert_eq!(generated.display_name(), from_json.display_name());
        assert_eq!(generated.language(), from_json.language());
        assert_eq!(parsed.name, name);
    }
}
