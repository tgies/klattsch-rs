//! ARPABET phoneme parameter table and the [`PhonemeTable`] trait.
//!
//! Constants source: Klatt, D.H. (1980), "Software for a cascade/parallel
//! formant synthesizer," *J. Acoust. Soc. Am.* 67(3), Tables II (vowels) and
//! III (consonants). Where Klatt gives two rows for a vowel, the second row
//! is the diphthong offglide endpoint, captured as [`PhonemeParams::glide_to`].
//! Amplitudes are approximated for our 3-formant parallel synth.

/// A diphthong's offglide endpoint. Overlays `f1`/`f2`/`f3` of the base
/// phoneme's formants when applied during synthesis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlideTo {
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
}

/// Phoneme-level synthesis parameters. Drives a [`PhonemeTable`] lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhonemeParams {
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
    pub bw1: f32,
    pub bw2: f32,
    pub bw3: f32,
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    /// 0..1; full-voiced sonorants are 1.0, voiceless fricatives 0.0,
    /// voiced fricatives ~0.45, voiced plosives ~0.6.
    pub voicing: f32,
    /// Diphthong offglide endpoint, if any.
    pub glide_to: Option<GlideTo>,
    /// Plosive / affricate: render as a brief silence followed by a burst.
    pub is_stop: bool,
}

/// Phoneme parameter source. Implementors provide name -> [`PhonemeParams`]
/// lookup. The default English ARPABET table is [`Arpabet`]; future language
/// tables (e.g. Japanese) plug in via this trait.
pub trait PhonemeTable: Send + Sync {
    /// Resolve a phoneme name (e.g. `"AH"`, `"UW"`) to its parameters.
    fn lookup(&self, name: &str) -> Option<PhonemeParams>;

    /// Iterator over all phoneme names known to this table.
    fn names(&self) -> &'static [&'static str];
}

/// English ARPABET (Klatt 1980 Tables II-III), 47 phonemes.
pub struct Arpabet;

/// Singleton instance of [`Arpabet`].
pub static ARPABET: Arpabet = Arpabet;

impl PhonemeTable for Arpabet {
    fn lookup(&self, name: &str) -> Option<PhonemeParams> {
        ARPABET_TABLE
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| *p)
    }

    fn names(&self) -> &'static [&'static str] {
        ARPABET_NAMES
    }
}

const fn vowel(
    f1: f32,
    f2: f32,
    f3: f32,
    bw1: f32,
    bw2: f32,
    bw3: f32,
    glide_to: Option<GlideTo>,
) -> PhonemeParams {
    PhonemeParams {
        f1,
        f2,
        f3,
        bw1,
        bw2,
        bw3,
        a1: 1.0,
        a2: 0.9,
        a3: 0.7,
        voicing: 1.0,
        glide_to,
        is_stop: false,
    }
}

const fn sonorant(f1: f32, f2: f32, f3: f32, bw1: f32, bw2: f32, bw3: f32) -> PhonemeParams {
    PhonemeParams {
        f1,
        f2,
        f3,
        bw1,
        bw2,
        bw3,
        a1: 0.8,
        a2: 0.7,
        a3: 0.5,
        voicing: 1.0,
        glide_to: None,
        is_stop: false,
    }
}

const fn fric_voiceless(f1: f32, f2: f32, f3: f32, a1: f32, a2: f32, a3: f32) -> PhonemeParams {
    PhonemeParams {
        f1,
        f2,
        f3,
        bw1: 200.0,
        bw2: 200.0,
        bw3: 1000.0,
        a1,
        a2,
        a3,
        voicing: 0.0,
        glide_to: None,
        is_stop: false,
    }
}

const fn fric_voiced(f1: f32, f2: f32, f3: f32, a1: f32, a2: f32, a3: f32) -> PhonemeParams {
    PhonemeParams {
        f1,
        f2,
        f3,
        bw1: 80.0,
        bw2: 100.0,
        bw3: 800.0,
        a1,
        a2,
        a3,
        voicing: 0.45,
        glide_to: None,
        is_stop: false,
    }
}

#[allow(clippy::too_many_arguments)]
const fn nasal(
    f1: f32,
    f2: f32,
    f3: f32,
    bw1: f32,
    bw2: f32,
    bw3: f32,
    a1: f32,
    a2: f32,
    a3: f32,
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
        voicing: 1.0,
        glide_to: None,
        is_stop: false,
    }
}

#[allow(clippy::too_many_arguments)]
const fn stop(
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
        glide_to: None,
        is_stop: true,
    }
}

const fn glide(f1: f32, f2: f32, f3: f32) -> Option<GlideTo> {
    Some(GlideTo { f1, f2, f3 })
}

#[rustfmt::skip]
static ARPABET_TABLE: &[(&str, PhonemeParams)] = &[
    // Vowels (Klatt 1980 Table II)
    ("IY", vowel(310.0, 2020.0, 2960.0,  45.0, 200.0, 400.0, glide(290.0, 2070.0, 2960.0))),
    ("IH", vowel(400.0, 1800.0, 2570.0,  50.0, 100.0, 140.0, glide(470.0, 1600.0, 2600.0))),
    ("EH", vowel(530.0, 1680.0, 2500.0,  60.0,  90.0, 200.0, glide(620.0, 1530.0, 2530.0))),
    ("AE", vowel(620.0, 1660.0, 2430.0,  70.0, 150.0, 320.0, glide(650.0, 1490.0, 2470.0))),
    ("AA", vowel(700.0, 1220.0, 2600.0, 130.0,  70.0, 160.0, None)),
    ("AO", vowel(600.0,  990.0, 2570.0,  90.0, 100.0,  80.0, glide(630.0, 1040.0, 2600.0))),
    ("AH", vowel(620.0, 1220.0, 2550.0,  80.0,  50.0, 140.0, None)),
    ("UH", vowel(450.0, 1100.0, 2350.0,  80.0, 100.0,  80.0, glide(500.0, 1180.0, 2390.0))),
    ("UW", vowel(350.0, 1250.0, 2200.0,  65.0, 110.0, 140.0, glide(320.0,  900.0, 2200.0))),
    ("ER", vowel(470.0, 1270.0, 1540.0, 100.0,  60.0, 110.0, glide(420.0, 1310.0, 1540.0))),

    // Diphthongs (Klatt 1980 Table II)
    ("AY", vowel(660.0, 1200.0, 2550.0, 100.0,  70.0, 200.0, glide(400.0, 1880.0, 2500.0))),
    ("AW", vowel(640.0, 1230.0, 2550.0,  80.0,  70.0, 140.0, glide(420.0,  940.0, 2350.0))),
    ("EY", vowel(480.0, 1720.0, 2520.0,  70.0, 100.0, 200.0, glide(330.0, 2020.0, 2600.0))),
    ("OW", vowel(540.0, 1100.0, 2300.0,  80.0,  70.0,  70.0, glide(450.0,  900.0, 2300.0))),
    ("OY", vowel(550.0,  960.0, 2400.0,  80.0,  50.0, 130.0, glide(360.0, 1820.0, 2450.0))),

    // Sonorants (Klatt 1980 Table III)
    ("W",  sonorant(290.0,  610.0, 2150.0,  50.0,  80.0,  60.0)),
    ("Y",  sonorant(260.0, 2070.0, 3020.0,  40.0, 250.0, 500.0)),
    ("R",  sonorant(310.0, 1060.0, 1380.0,  70.0, 100.0, 120.0)),
    ("L",  sonorant(310.0, 1050.0, 2880.0,  50.0, 100.0, 280.0)),

    // Nasals (Klatt 1980 Table III, approximated)
    ("M",  nasal(270.0, 1270.0, 2130.0, 40.0, 200.0, 200.0, 0.7, 0.18, 0.10)),
    ("N",  nasal(270.0, 1340.0, 2470.0, 40.0, 300.0, 300.0, 0.7, 0.20, 0.12)),
    ("NG", nasal(270.0, 2000.0, 2700.0, 40.0, 300.0, 300.0, 0.7, 0.20, 0.12)),

    // Voiceless fricatives (Klatt 1980 Table III)
    ("F",  fric_voiceless(340.0, 1100.0, 2080.0, 0.0, 0.10, 0.15)),
    ("TH", fric_voiceless(320.0, 1290.0, 2540.0, 0.0, 0.08, 0.18)),
    ("S",  fric_voiceless(320.0, 1390.0, 5500.0, 0.0, 0.0,  0.95)),
    ("SH", fric_voiceless(300.0, 1840.0, 2750.0, 0.0, 0.55, 0.65)),

    // Voiced fricatives
    ("V",  fric_voiced(220.0, 1100.0, 2080.0, 0.4, 0.12, 0.18)),
    ("DH", fric_voiced(270.0, 1290.0, 2540.0, 0.4, 0.10, 0.20)),
    ("Z",  fric_voiced(240.0, 1390.0, 5500.0, 0.4, 0.0,  0.65)),
    ("ZH", fric_voiced(270.0, 1840.0, 2750.0, 0.4, 0.45, 0.55)),

    // /h/ aspirated fricative
    ("HH", PhonemeParams {
        f1: 500.0, f2: 1500.0, f3: 2500.0,
        bw1: 300.0, bw2: 200.0, bw3: 300.0,
        a1: 0.4, a2: 0.4, a3: 0.3,
        voicing: 0.0, glide_to: None, is_stop: false,
    }),

    // Plosives (burst spectra from Klatt 1980 Table III)
    ("P", stop(0.0,  400.0, 1100.0, 2150.0, 300.0, 150.0, 220.0, 0.10, 0.20, 0.25)),
    ("B", stop(0.6,  200.0, 1100.0, 2150.0,  60.0, 110.0, 130.0, 0.50, 0.20, 0.20)),
    ("T", stop(0.0,  400.0, 1600.0, 2600.0, 300.0, 120.0, 250.0, 0.0,  0.30, 0.55)),
    ("D", stop(0.6,  200.0, 1600.0, 2600.0,  60.0, 100.0, 170.0, 0.50, 0.40, 0.50)),
    ("K", stop(0.0,  300.0, 1990.0, 2850.0, 250.0, 160.0, 330.0, 0.0,  0.50, 0.40)),
    ("G", stop(0.6,  200.0, 1990.0, 2850.0,  60.0, 150.0, 280.0, 0.50, 0.50, 0.40)),

    // Affricates
    ("CH", stop(0.0, 350.0, 1800.0, 2820.0, 200.0,  90.0, 300.0, 0.0,  0.40, 0.55)),
    ("JH", stop(0.5, 260.0, 1800.0, 2820.0,  60.0,  80.0, 270.0, 0.40, 0.40, 0.50)),

    // Silence (just zero amplitudes; defaults for the rest)
    ("_",  PhonemeParams {
        f1: 500.0, f2: 1500.0, f3: 2500.0,
        bw1: 80.0, bw2: 120.0, bw3: 160.0,
        a1: 0.0, a2: 0.0, a3: 0.0,
        voicing: 0.0, glide_to: None, is_stop: false,
    }),
];

static ARPABET_NAMES: &[&str] = &[
    "IY", "IH", "EH", "AE", "AA", "AO", "AH", "UH", "UW", "ER", "AY", "AW", "EY", "OW", "OY", "W",
    "Y", "R", "L", "M", "N", "NG", "F", "TH", "S", "SH", "V", "DH", "Z", "ZH", "HH", "P", "B", "T",
    "D", "K", "G", "CH", "JH", "_",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arpabet_table_size_matches_js() {
        // 10 vowels + 5 diphthongs + 4 sonorants + 3 nasals + 4 voiceless fric
        // + 4 voiced fric + 1 HH + 6 plosives + 2 affricates + 1 silence = 40
        // (the JS PHONEME_KEYS count excludes the underscore)
        assert_eq!(ARPABET_TABLE.len(), 40);
        assert_eq!(ARPABET_NAMES.len(), 40);
    }

    #[test]
    fn lookup_known_phonemes() {
        let table = &ARPABET as &dyn PhonemeTable;
        let ah = table.lookup("AH").expect("AH should exist");
        assert_eq!(ah.f1, 620.0);
        assert_eq!(ah.f2, 1220.0);
        assert!(!ah.is_stop);
        assert!(ah.glide_to.is_none());

        let p = table.lookup("P").expect("P should exist");
        assert!(p.is_stop);

        let iy = table.lookup("IY").expect("IY should exist");
        let g = iy.glide_to.expect("IY has glide_to");
        assert_eq!(g.f1, 290.0);
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let table = &ARPABET as &dyn PhonemeTable;
        assert!(table.lookup("XYZZY").is_none());
        assert!(table.lookup("").is_none());
    }

    #[test]
    fn voiced_fricatives_have_bumped_a1() {
        // A1 in JS voicedFric = caller_a1 + voicedAmp*0.8 = 0 + 0.5*0.8 = 0.4
        for name in ["V", "DH", "Z", "ZH"] {
            let p = ARPABET.lookup(name).unwrap();
            assert!(
                (p.a1 - 0.4).abs() < 1e-6,
                "{name} a1 = {} should be 0.4",
                p.a1
            );
            assert!((p.voicing - 0.45).abs() < 1e-6);
        }
    }

    #[test]
    fn voiced_fricatives_have_voiced_bandwidths() {
        // BW1=80, BW2=100, BW3=800 (vs voiceless 200/200/1000)
        let v = ARPABET.lookup("V").unwrap();
        assert_eq!(v.bw1, 80.0);
        assert_eq!(v.bw2, 100.0);
        assert_eq!(v.bw3, 800.0);
    }

    #[test]
    fn all_names_resolve() {
        for n in ARPABET.names() {
            assert!(ARPABET.lookup(n).is_some(), "{n} should resolve");
        }
    }
}
