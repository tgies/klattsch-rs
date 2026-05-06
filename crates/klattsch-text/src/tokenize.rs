//! Lex a phoneme-string source into a stream of [`Token`]s. Whitespace-
//! separated; supports `#` line comments (at a token boundary) and `/* */`
//! block comments.

use crate::normalize::normalize;

/// Source span of a token, byte offsets into the normalized source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectiveKey {
    /// Base pitch in Hz. Letter `b`.
    Base,
    /// Per-phoneme duration in ms. Letter `r`.
    Rate,
    /// Pause duration in ms (only valid when a value is supplied). Letter `p`.
    Pause,
    /// Formant frequency / bandwidth scale factor. Letter `s`.
    Scale,
    /// Vibrato depth in Hz. Letter `v`.
    Vibrato,
    /// Vibrato rate in Hz. Letter `w`.
    VibratoRate,
    /// Tremolo depth (0..1). Letter `m`.
    Tremolo,
    /// Tremolo rate in Hz. Letter `n`.
    TremoloRate,
    /// Aspiration / breathiness (0..1). Letter `h`.
    Aspiration,
    /// Spectral tilt (-0.95..0.95). Letter `t`.
    Tilt,
    /// Glottal effort (0..1, lax..tense). Letter `g`.
    Effort,
}

impl DirectiveKey {
    /// Map the single-letter directive prefix to a `DirectiveKey`.
    pub fn from_letter(c: char) -> Option<Self> {
        Some(match c {
            'b' => Self::Base,
            'r' => Self::Rate,
            'p' => Self::Pause,
            's' => Self::Scale,
            'v' => Self::Vibrato,
            'w' => Self::VibratoRate,
            'm' => Self::Tremolo,
            'n' => Self::TremoloRate,
            'h' => Self::Aspiration,
            't' => Self::Tilt,
            'g' => Self::Effort,
            _ => return None,
        })
    }

    /// Map the bracket-form directive name (`[base=120]` style) to a key.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "base" | "pitch" => Self::Base,
            "rate" => Self::Rate,
            "pause" => Self::Pause,
            "scale" => Self::Scale,
            "vibrato" => Self::Vibrato,
            "vibratoRate" => Self::VibratoRate,
            "tremolo" => Self::Tremolo,
            "tremoloRate" => Self::TremoloRate,
            "aspiration" => Self::Aspiration,
            "tilt" => Self::Tilt,
            "effort" => Self::Effort,
            _ => return None,
        })
    }
}

/// Lexed token from the input. Spans are byte offsets into the normalized
/// source.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    /// An ARPABET phoneme reference, possibly stressed, possibly with a sticky
    /// or transient pitch delta.
    Phoneme {
        code: String,
        stressed: bool,
        pitch_delta: f32,
        /// `true` for `(+30)` form (only affects this phoneme), `false` for
        /// the `+30` sticky form (delta persists into subsequent F0 baseline).
        transient: bool,
        span: Span,
    },
    /// A directive. `value` is meaningful only when `reset` is `false`.
    /// `relative` is `true` for the `g+0.1` / `g-0.3` form (delta against
    /// current). `false` for the explicit `g=0.5` form or the bare numeric
    /// `g0.5` form (which is absolute by default).
    Directive {
        key: DirectiveKey,
        value: f32,
        relative: bool,
        /// `true` for a bare letter (e.g. `g` alone), meaning "reset to the
        /// initial value supplied via [`crate::CompileOptions`]".
        reset: bool,
        span: Span,
    },
    /// A pause from the punctuation forms `,` `;` `.` (durations 100/200/300
    /// ms).
    Pause { ms: f32, span: Span },
    /// `(` opens a syllable group; tokens until the matching `)` are queued
    /// and rendered together as one syllable, sharing the rate slot.
    SyllableOpen { span: Span },
    /// `)` closes the current syllable group.
    SyllableClose { span: Span },
    /// A token that didn't match any recognized form. The compiler emits a
    /// warning and skips it (matches JS sequencer behavior).
    Unknown { text: String, span: Span },
}

/// Tokenizer error. Anything unrecognized becomes a [`Token::Unknown`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {}

impl core::fmt::Display for ParseError {
    fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for ParseError {}

/// Tokenized source: the original normalized string plus its lex.
#[derive(Clone, Debug)]
pub struct Tokenized {
    pub source: String,
    pub tokens: Vec<Token>,
}

const PAUSE_COMMA_MS: f32 = 100.0;
const PAUSE_SEMI_MS: f32 = 200.0;
const PAUSE_PERIOD_MS: f32 = 300.0;

/// Lex `input` into a [`Tokenized`] result.
pub fn tokenize(input: &str) -> Result<Tokenized, ParseError> {
    let source = normalize(input);
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0;

    let find_block_end = |start: usize| -> usize {
        // Look for `*/` after the opening `/*`. start points at the opening
        // `/`.
        let s = &source[start + 2..];
        match s.find("*/") {
            Some(off) => start + 2 + off + 2,
            None => len, // unterminated -> consume to EOF
        }
    };

    while i < len {
        let c = bytes[i];
        if (c as char).is_whitespace() {
            i += 1;
            continue;
        }
        // Line comment: `#` only at a token boundary (start or after
        // whitespace).
        let prev_is_ws = i == 0 || (bytes[i - 1] as char).is_whitespace();
        if c == b'#' && prev_is_ws {
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment.
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i = find_block_end(i);
            continue;
        }

        let src_start = i;
        let mut part = String::new();
        while i < len && !(bytes[i] as char).is_whitespace() {
            // Embedded block comment inside a token still terminates it (per
            // JS).
            if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                i = find_block_end(i);
                continue;
            }
            part.push(bytes[i] as char);
            i += 1;
        }
        let src_end = i;
        if part.is_empty() {
            continue;
        }
        let span = Span {
            start: src_start,
            end: src_end,
        };

        // Stress mark applied to previous phoneme.
        if part == "!" || part == "'" {
            for tok in tokens.iter_mut().rev() {
                if let Token::Phoneme { stressed, .. } = tok {
                    *stressed = true;
                    break;
                }
            }
            continue;
        }

        match classify(&part, span) {
            Some(tok) => tokens.push(tok),
            None => continue, // bare `p` is silently dropped (matches JS)
        }
    }

    Ok(Tokenized { source, tokens })
}

fn classify(part: &str, span: Span) -> Option<Token> {
    if part == "(" {
        return Some(Token::SyllableOpen { span });
    }
    if part == ")" {
        return Some(Token::SyllableClose { span });
    }
    if part == "," {
        return Some(Token::Pause {
            ms: PAUSE_COMMA_MS,
            span,
        });
    }
    if part == ";" {
        return Some(Token::Pause {
            ms: PAUSE_SEMI_MS,
            span,
        });
    }
    if part == "." {
        return Some(Token::Pause {
            ms: PAUSE_PERIOD_MS,
            span,
        });
    }

    let unknown = || {
        Some(Token::Unknown {
            text: part.into(),
            span,
        })
    };

    // Bracket directive: [name=value]
    if let Some(rest) = part.strip_prefix('[') {
        if let Some(inner) = rest.strip_suffix(']') {
            if let Some((name, value_str)) = inner.split_once('=') {
                if let (Ok(value), Some(key)) =
                    (value_str.parse::<f32>(), DirectiveKey::from_name(name))
                {
                    return Some(Token::Directive {
                        key,
                        value,
                        relative: false,
                        reset: false,
                        span,
                    });
                }
            }
            return unknown();
        }
    }

    // Note-form pitch directive: `bC4`, `b=Bb-1`
    let chars: Vec<char> = part.chars().collect();
    if chars.first() == Some(&'b') && chars.len() >= 2 {
        let after_b_offset = if chars.get(1) == Some(&'=') { 2 } else { 1 };
        if chars.len() > after_b_offset {
            let note_str: String = chars[after_b_offset..].iter().collect();
            if let Some(hz) = note_to_hz(&note_str) {
                return Some(Token::Directive {
                    key: DirectiveKey::Base,
                    value: hz,
                    relative: false,
                    reset: false,
                    span,
                });
            }
        }
    }

    // Compact directive: single lowercase letter optionally followed by
    // `=value` or a signed/unsigned number. Bare letter = reset.
    if let Some(first) = chars.first() {
        if first.is_ascii_lowercase() {
            if let Some(key) = DirectiveKey::from_letter(*first) {
                if chars.len() == 1 {
                    // Bare letter. JS: drop bare `p`, otherwise mark reset.
                    if key == DirectiveKey::Pause {
                        return None;
                    }
                    return Some(Token::Directive {
                        key,
                        value: 0.0,
                        relative: false,
                        reset: true,
                        span,
                    });
                }
                let mut idx = 1;
                let explicit_eq = chars.get(idx) == Some(&'=');
                if explicit_eq {
                    idx += 1;
                }
                let rest: String = chars[idx..].iter().collect();
                if let Ok(value) = rest.parse::<f32>() {
                    let leading = chars.get(idx);
                    let signed = matches!(leading, Some('+') | Some('-'));
                    let relative = !explicit_eq && signed;
                    return Some(Token::Directive {
                        key,
                        value,
                        relative,
                        reset: false,
                        span,
                    });
                }
            }
        }
    }

    // Phoneme: ARPABET letters, optional stress mark, optional pitch delta
    // (transient `(+30)` or sticky `+30`).
    if chars.first().is_some_and(|c| c.is_ascii_uppercase()) {
        let mut idx = 0;
        let code_start = idx;
        while idx < chars.len() && chars[idx].is_ascii_uppercase() {
            idx += 1;
        }
        let code: String = chars[code_start..idx].iter().collect();

        let mut stressed = false;
        if let Some(c) = chars.get(idx) {
            if *c == '\'' || *c == '!' {
                stressed = true;
                idx += 1;
            }
        }

        let mut pitch_delta = 0.0f32;
        let mut transient = false;
        if let Some(c) = chars.get(idx) {
            if *c == '(' {
                // Transient: `(+30)` or `(-12.5)`
                let close = chars[idx + 1..]
                    .iter()
                    .position(|c| *c == ')')
                    .map(|p| idx + 1 + p);
                if let Some(close_idx) = close {
                    let inner: String = chars[idx + 1..close_idx].iter().collect();
                    if matches!(inner.chars().next(), Some('+') | Some('-')) {
                        if let Ok(v) = inner.parse::<f32>() {
                            pitch_delta = v;
                            transient = true;
                            idx = close_idx + 1;
                        }
                    }
                }
            } else if *c == '+' || *c == '-' {
                let rest: String = chars[idx..].iter().collect();
                if let Ok(v) = rest.parse::<f32>() {
                    pitch_delta = v;
                    transient = false;
                    idx = chars.len();
                }
            }
        }

        if idx == chars.len() {
            return Some(Token::Phoneme {
                code,
                stressed,
                pitch_delta,
                transient,
                span,
            });
        }
    }

    unknown()
}

/// Convert a note name (e.g. `"C4"`, `"Bb-1"`) to Hz via standard MIDI math.
/// Mirrors `noteToHz` in `sequencer.js:9-19`.
pub fn note_to_hz(name: &str) -> Option<f32> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let letter = match bytes[0] {
        b'C' => 0i32,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let mut idx = 1;
    let mut accidental = 0i32;
    if idx < bytes.len() {
        match bytes[idx] {
            b'#' => {
                accidental = 1;
                idx += 1;
            }
            b'b' => {
                accidental = -1;
                idx += 1;
            }
            _ => {}
        }
    }
    if idx >= bytes.len() {
        return None;
    }
    let octave_str = &name[idx..];
    let octave: i32 = octave_str.parse().ok()?;
    let semi = letter + accidental;
    let midi = (octave + 1) * 12 + semi;
    Some(440.0 * 2.0_f32.powf((midi - 69) as f32 / 12.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_phonemes() {
        let r = tokenize("HH AH L OW").unwrap();
        assert_eq!(r.tokens.len(), 4);
        match &r.tokens[0] {
            Token::Phoneme { code, .. } => assert_eq!(code, "HH"),
            other => panic!("expected phoneme, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_pause_punctuation() {
        let r = tokenize("AH , AH . AH").unwrap();
        let pauses: Vec<f32> = r
            .tokens
            .iter()
            .filter_map(|t| match t {
                Token::Pause { ms, .. } => Some(*ms),
                _ => None,
            })
            .collect();
        assert_eq!(pauses, vec![100.0, 300.0]);
    }

    #[test]
    fn tokenize_directives() {
        let r = tokenize("b120 r60 g0.7").unwrap();
        assert_eq!(r.tokens.len(), 3);
        match &r.tokens[0] {
            Token::Directive {
                key: DirectiveKey::Base,
                value,
                relative,
                reset,
                ..
            } => {
                assert_eq!(*value, 120.0);
                assert!(!*relative);
                assert!(!*reset);
            }
            other => panic!("expected base directive, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_relative_directive() {
        let r = tokenize("g+0.2").unwrap();
        match &r.tokens[0] {
            Token::Directive {
                value, relative, ..
            } => {
                assert!((*value - 0.2).abs() < 1e-6);
                assert!(*relative);
            }
            other => panic!("expected directive, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_explicit_equals_directive_is_absolute() {
        let r = tokenize("g=0.7").unwrap();
        match &r.tokens[0] {
            Token::Directive {
                value, relative, ..
            } => {
                assert_eq!(*value, 0.7);
                assert!(!*relative);
            }
            other => panic!("expected directive, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_bare_letter_is_reset() {
        let r = tokenize("b").unwrap();
        match &r.tokens[0] {
            Token::Directive { reset, .. } => assert!(*reset),
            other => panic!("expected directive, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_bare_p_is_dropped() {
        let r = tokenize("AH p AH").unwrap();
        // Bare `p` produces no token, leaving 2 phonemes
        assert_eq!(r.tokens.len(), 2);
    }

    #[test]
    fn tokenize_note_directive() {
        let r = tokenize("bC4").unwrap();
        match &r.tokens[0] {
            Token::Directive {
                key: DirectiveKey::Base,
                value,
                ..
            } => {
                assert!((*value - 261.625_55).abs() < 0.01);
            }
            other => panic!("expected base directive, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_phoneme_sticky_delta() {
        let r = tokenize("AY+30").unwrap();
        match &r.tokens[0] {
            Token::Phoneme {
                code,
                pitch_delta,
                transient,
                ..
            } => {
                assert_eq!(code, "AY");
                assert_eq!(*pitch_delta, 30.0);
                assert!(!*transient);
            }
            other => panic!("expected phoneme, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_phoneme_transient_delta() {
        let r = tokenize("AY(+30)").unwrap();
        match &r.tokens[0] {
            Token::Phoneme {
                pitch_delta,
                transient,
                ..
            } => {
                assert_eq!(*pitch_delta, 30.0);
                assert!(*transient);
            }
            other => panic!("expected phoneme, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_stress_mark_attaches_to_previous() {
        let r = tokenize("AH ! AH").unwrap();
        let stressed: Vec<bool> = r
            .tokens
            .iter()
            .filter_map(|t| match t {
                Token::Phoneme { stressed, .. } => Some(*stressed),
                _ => None,
            })
            .collect();
        assert_eq!(stressed, vec![true, false]);
    }

    #[test]
    fn tokenize_skips_line_comment() {
        let r = tokenize("# this is a comment\nHH AH").unwrap();
        assert_eq!(r.tokens.len(), 2);
    }

    #[test]
    fn tokenize_skips_block_comment() {
        let r = tokenize("HH /* note */ AH").unwrap();
        assert_eq!(r.tokens.len(), 2);
    }

    #[test]
    fn tokenize_syllable_grouping() {
        let r = tokenize("( HH AH )").unwrap();
        assert!(matches!(r.tokens[0], Token::SyllableOpen { .. }));
        assert!(matches!(r.tokens[3], Token::SyllableClose { .. }));
    }

    #[test]
    fn tokenize_unknown_emits_unknown_token() {
        let r = tokenize("xyz123").unwrap();
        assert_eq!(r.tokens.len(), 1);
        assert!(matches!(r.tokens[0], Token::Unknown { .. }));
    }

    #[test]
    fn note_to_hz_a4() {
        assert!((note_to_hz("A4").unwrap() - 440.0).abs() < 1e-3);
    }

    #[test]
    fn note_to_hz_c4_middle_c() {
        assert!((note_to_hz("C4").unwrap() - 261.625_55).abs() < 0.01);
    }

    #[test]
    fn note_to_hz_with_accidentals() {
        assert!(note_to_hz("C#4").unwrap() > note_to_hz("C4").unwrap());
        assert!(note_to_hz("Bb4").unwrap() < note_to_hz("B4").unwrap());
    }
}
