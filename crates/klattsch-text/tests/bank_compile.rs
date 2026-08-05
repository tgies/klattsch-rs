//! Compiling against a phoneme bank rather than the built-in ARPABET table.
//!
//! Banks are only useful if they reach the compiler, so these drive the real
//! path: bank -> `CompileOptions::phoneme_table` -> schedule -> rendered
//! audio.

use klattsch_core::banks::BankRegistry;
use klattsch_core::{FormantSynth, PhonemeTable, ARPABET};
use klattsch_text::{compile_string, CompileOptions};

fn render(source: &str, table: &dyn PhonemeTable) -> Vec<f32> {
    let result = compile_string(
        source,
        &CompileOptions {
            phoneme_table: table,
            ..CompileOptions::default()
        },
    )
    .expect("compiles");
    let mut synth = FormantSynth::new(48_000);
    synth.queue_schedule(result.schedule);
    let mut out = vec![0.0; 48_000];
    synth.process(&mut out);
    out
}

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

#[test]
fn a_bank_drives_the_compiler() {
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("klatt1980-en").unwrap();
    let audio = render("HH AH L OW", bank);
    assert!(rms(&audio) > 0.01, "a bank-compiled phrase sounds");
}

#[test]
fn japanese_phonemes_only_resolve_in_a_japanese_bank() {
    let reg = BankRegistry::with_bundled();
    let ja = reg.get("ja-mokhtari-2000").unwrap();

    // Single-letter Japanese vowel names tokenize unambiguously: the lexer
    // takes a maximal uppercase run and tokens are whitespace-separated, so
    // `A` and `AA` cannot be confused.
    let audio = render("K O N N I CH I W A", ja);
    assert!(rms(&audio) > 0.01, "a Japanese phrase sounds");

    assert!(ja.lookup("A").is_some(), "the Japanese vowel exists");
    assert!(
        ARPABET.lookup("A").is_none(),
        "and is absent from the English table, so the bank is doing the work"
    );
    // The consonants come from the inherited English set.
    assert_eq!(ja.lookup("K"), ARPABET.lookup("K"));
}

#[test]
fn the_two_japanese_tunings_render_differently() {
    let reg = BankRegistry::with_bundled();
    let mokhtari = render("A I U E O", reg.get("ja-mokhtari-2000").unwrap());
    let hecko = render("A I U E O", reg.get("ja-hecko-2026").unwrap());
    assert!(rms(&mokhtari) > 0.01 && rms(&hecko) > 0.01);
    assert_ne!(
        mokhtari, hecko,
        "different vowel tunings must produce different audio"
    );
}

#[test]
fn the_english_bank_matches_the_built_in_table() {
    // The bundled English bank and the hardcoded ARPABET table are two
    // encodings of the same Klatt 1980 data; they must agree, or switching a
    // caller from one to the other would change its sound.
    let reg = BankRegistry::with_bundled();
    let bank = reg.get("klatt1980-en").unwrap();
    for name in ARPABET.names() {
        assert_eq!(
            bank.lookup(name),
            ARPABET.lookup(name),
            "{name} differs between the bundled bank and the built-in table"
        );
    }
    assert_eq!(bank.name_list(), ARPABET.names().to_vec());
    assert_eq!(render("HH AH L OW", bank), render("HH AH L OW", &ARPABET));
}
