//! Input normalization: NFKC + zero-width strip + Latin-lookalike fold, so
//! pasted-from-doc input parses the same as keyboard-typed input.

use unicode_normalization::UnicodeNormalization;

/// Apply NFKC, strip zero-width formatting characters, and fold a curated set
/// of Greek/Cyrillic homoglyphs to their Latin lookalikes.
pub fn normalize(input: &str) -> String {
    let nfkc: String = input.nfkc().collect();
    nfkc.chars()
        .filter(|c| !is_zero_width(*c))
        .map(fold_homoglyph)
        .collect()
}

const fn is_zero_width(c: char) -> bool {
    matches!(c as u32, 0x200B | 0x200C | 0x200D | 0x2060 | 0xFEFF)
}

fn fold_homoglyph(c: char) -> char {
    match c {
        // Greek uppercase
        'Α' => 'A',
        'Β' => 'B',
        'Ε' => 'E',
        'Η' => 'H',
        'Ι' => 'I',
        'Κ' => 'K',
        'Μ' => 'M',
        'Ν' => 'N',
        'Ο' => 'O',
        'Ρ' => 'P',
        'Τ' => 'T',
        'Υ' => 'Y',
        'Ζ' => 'Z',
        // Cyrillic uppercase
        'А' => 'A',
        'В' => 'B',
        'С' => 'C',
        'Е' => 'E',
        'Н' => 'H',
        'К' => 'K',
        'М' => 'M',
        'О' => 'O',
        'Р' => 'P',
        'Т' => 'T',
        // Cyrillic lowercase
        'а' => 'a',
        'с' => 'c',
        'е' => 'e',
        'о' => 'o',
        'р' => 'p',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        assert_eq!(normalize("HH AH L OW"), "HH AH L OW");
    }

    #[test]
    fn strips_zero_width_joiner() {
        let s = format!("AH{}AH", '\u{200D}');
        assert_eq!(normalize(&s), "AHAH");
    }

    #[test]
    fn folds_cyrillic_lookalikes() {
        // Cyrillic А (U+0410) and С (U+0421) should fold to Latin A and C.
        assert_eq!(normalize("\u{0410}\u{0421}"), "AC");
    }

    #[test]
    fn nfkc_normalizes_compat_chars() {
        // Fullwidth A (U+FF21) should NFKC-fold to plain A.
        assert_eq!(normalize("\u{FF21}H"), "AH");
    }
}
