//! Compile a [`Tokenized`] phoneme string into a [`Schedule`] plus phrase
//! markers and a warnings list.

use klattsch_core::phonemes::{GlideTo, PhonemeParams};
use klattsch_core::{MsEvent, ParamUpdate, PhonemeTable, Schedule, ARPABET};

use crate::tokenize::{DirectiveKey, Span, Token, Tokenized};

pub struct CompileOptions<'a> {
    pub sample_rate: u32,
    pub base_f0: f32,
    pub rate: f32,
    pub scale: f32,
    pub vibrato_depth: f32,
    pub vibrato_rate: f32,
    pub tremolo_depth: f32,
    pub tremolo_rate: f32,
    pub aspiration: f32,
    pub tilt: f32,
    pub effort: f32,
    pub default_transition_ms: f32,
    pub phoneme_table: &'a dyn PhonemeTable,
}

impl Default for CompileOptions<'_> {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            base_f0: 120.0,
            rate: 110.0,
            scale: 1.0,
            vibrato_depth: 0.0,
            vibrato_rate: 5.0,
            tremolo_depth: 0.0,
            tremolo_rate: 5.0,
            aspiration: 0.0,
            tilt: 0.0,
            effort: 0.5,
            default_transition_ms: 35.0,
            phoneme_table: &ARPABET,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    UnclosedSyllableGroup { span: Span },
    UnmatchedSyllableClose { span: Span },
}

impl core::fmt::Display for CompileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnclosedSyllableGroup { span } => {
                write!(f, "unclosed `(` at offset {}", span.start)
            }
            Self::UnmatchedSyllableClose { span } => {
                write!(f, "unmatched `)` at offset {}", span.start)
            }
        }
    }
}

impl std::error::Error for CompileError {}

#[derive(Clone, Debug)]
pub struct CompileResult {
    pub schedule: Schedule,
    pub total_ms: f32,
    pub warnings: Vec<String>,
    pub phrases: Vec<Phrase>,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct Phrase {
    pub src_start: usize,
    pub src_end: usize,
    pub token_src_start: usize,
    pub t_start_ms: f32,
    pub t_end_ms: f32,
    pub kind: PhraseKind,
    pub phoneme: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseKind {
    Phoneme,
    Pause,
    Directive,
}

const STRESS_DURATION_FACTOR: f32 = 1.5;
const STRESS_F0_LIFT: f32 = 8.0;
const STOP_BURST_MS: f32 = 25.0;
const SENTENCE_FINAL_HOLD_MS: f32 = 0.0;
const FADE_OUT_MS: f32 = 100.0;
const TRAIL_OFF_MS: f32 = 150.0;
const SILENCE_TRANSITION_MS: f32 = 30.0;

#[derive(Clone, Copy)]
struct VoiceExtras {
    vibrato_depth: f32,
    vibrato_rate: f32,
    tremolo_depth: f32,
    tremolo_rate: f32,
    aspiration: f32,
    tilt: f32,
    effort: f32,
}

/// Compile a tokenized utterance into a schedule.
pub fn compile(
    tokenized: &Tokenized,
    opts: &CompileOptions<'_>,
) -> Result<CompileResult, CompileError> {
    let mut state = State::new(opts);
    let mut events: Vec<MsEvent> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut phrases: Vec<Phrase> = Vec::new();

    let mut phrase_src_start: usize = 0;
    let mut phrase_time_start: f32 = 0.0;

    let mut in_syllable = false;
    let mut syllable_queue: Vec<Token> = Vec::new();

    for tok in &tokenized.tokens {
        match tok {
            Token::Unknown { text, .. } => {
                warnings.push(format!("unknown token: {text}"));
            }
            Token::SyllableOpen { span } => {
                if in_syllable {
                    warnings.push("nested ( ignored".into());
                    continue;
                }
                let _ = span; // span retained on the token; nothing to do here
                in_syllable = true;
                syllable_queue.clear();
            }
            Token::SyllableClose { span } => {
                if !in_syllable {
                    return Err(CompileError::UnmatchedSyllableClose { span: *span });
                }
                flush_syllable(
                    &mut state,
                    &mut events,
                    &mut warnings,
                    &mut phrases,
                    &mut phrase_src_start,
                    &mut phrase_time_start,
                    &mut syllable_queue,
                    opts,
                );
                in_syllable = false;
            }
            Token::Directive {
                key,
                value,
                relative,
                reset,
                span,
            } => {
                apply_directive(
                    &mut state,
                    *key,
                    *value,
                    *relative,
                    *reset,
                    *span,
                    &mut events,
                    &mut warnings,
                    &mut phrases,
                    &mut phrase_src_start,
                    &mut phrase_time_start,
                    opts,
                );
            }
            Token::Pause { ms, span } => {
                push_silence(&mut events, &state);
                state.time_ms += *ms;
                phrases.push(Phrase {
                    src_start: phrase_src_start,
                    src_end: span.end,
                    token_src_start: span.start,
                    t_start_ms: phrase_time_start,
                    t_end_ms: state.time_ms,
                    kind: PhraseKind::Pause,
                    phoneme: None,
                });
                phrase_src_start = span.end;
                phrase_time_start = state.time_ms;
            }
            Token::Phoneme { .. } => {
                if in_syllable {
                    syllable_queue.push(tok.clone());
                    continue;
                }
                render_one_phoneme(
                    tok,
                    &mut state,
                    &mut events,
                    &mut warnings,
                    &mut phrases,
                    &mut phrase_src_start,
                    &mut phrase_time_start,
                    opts,
                );
            }
        }
    }

    if in_syllable {
        warnings.push("unclosed (".into());
        flush_syllable(
            &mut state,
            &mut events,
            &mut warnings,
            &mut phrases,
            &mut phrase_src_start,
            &mut phrase_time_start,
            &mut syllable_queue,
            opts,
        );
    }

    state.time_ms += SENTENCE_FINAL_HOLD_MS;
    push_silence_with_transition(&mut events, &state, FADE_OUT_MS);
    state.time_ms += TRAIL_OFF_MS;

    if let Some(last) = phrases.last_mut() {
        last.t_end_ms = state.time_ms;
    }

    let schedule = Schedule::from_ms_events(opts.sample_rate, events);

    Ok(CompileResult {
        schedule,
        total_ms: state.time_ms,
        warnings,
        phrases,
        source: tokenized.source.clone(),
    })
}

/// Convenience: tokenize then compile.
pub fn compile_string(
    input: &str,
    opts: &CompileOptions<'_>,
) -> Result<CompileResult, crate::CompileOrParseError> {
    let toks = crate::tokenize::tokenize(input).map_err(crate::CompileOrParseError::Parse)?;
    compile(&toks, opts).map_err(crate::CompileOrParseError::Compile)
}

struct State {
    time_ms: f32,
    f0: f32,
    rate: f32,
    scale: f32,
    extras: VoiceExtras,
}

impl State {
    fn new(opts: &CompileOptions<'_>) -> Self {
        Self {
            time_ms: 0.0,
            f0: opts.base_f0,
            rate: opts.rate,
            scale: opts.scale,
            extras: VoiceExtras {
                vibrato_depth: opts.vibrato_depth,
                vibrato_rate: opts.vibrato_rate,
                tremolo_depth: opts.tremolo_depth,
                tremolo_rate: opts.tremolo_rate,
                aspiration: opts.aspiration,
                tilt: opts.tilt,
                effort: opts.effort,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_directive(
    state: &mut State,
    key: DirectiveKey,
    value: f32,
    relative: bool,
    reset: bool,
    span: Span,
    events: &mut Vec<MsEvent>,
    _warnings: &mut Vec<String>,
    phrases: &mut Vec<Phrase>,
    phrase_src_start: &mut usize,
    phrase_time_start: &mut f32,
    opts: &CompileOptions<'_>,
) {
    macro_rules! upd {
        ($field:ident, $initial:expr) => {{
            let initial = $initial;
            if reset {
                state.$field = initial;
            } else if relative {
                state.$field += value;
            } else {
                state.$field = value;
            }
        }};
        ($place:expr, $initial:expr, raw) => {{
            let initial = $initial;
            if reset {
                *$place = initial;
            } else if relative {
                *$place += value;
            } else {
                *$place = value;
            }
        }};
    }
    match key {
        DirectiveKey::Base => upd!(f0, opts.base_f0),
        DirectiveKey::Rate => upd!(rate, opts.rate),
        DirectiveKey::Scale => upd!(scale, opts.scale),
        DirectiveKey::Vibrato => upd!(&mut state.extras.vibrato_depth, opts.vibrato_depth, raw),
        DirectiveKey::VibratoRate => upd!(&mut state.extras.vibrato_rate, opts.vibrato_rate, raw),
        DirectiveKey::Tremolo => upd!(&mut state.extras.tremolo_depth, opts.tremolo_depth, raw),
        DirectiveKey::TremoloRate => upd!(&mut state.extras.tremolo_rate, opts.tremolo_rate, raw),
        DirectiveKey::Aspiration => upd!(&mut state.extras.aspiration, opts.aspiration, raw),
        DirectiveKey::Tilt => upd!(&mut state.extras.tilt, opts.tilt, raw),
        DirectiveKey::Effort => upd!(&mut state.extras.effort, opts.effort, raw),
        DirectiveKey::Pause => {
            push_silence(events, state);
            state.time_ms += value.abs();
            phrases.push(Phrase {
                src_start: *phrase_src_start,
                src_end: span.end,
                token_src_start: span.start,
                t_start_ms: *phrase_time_start,
                t_end_ms: state.time_ms,
                kind: PhraseKind::Pause,
                phoneme: None,
            });
            *phrase_src_start = span.end;
            *phrase_time_start = state.time_ms;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_one_phoneme(
    tok: &Token,
    state: &mut State,
    events: &mut Vec<MsEvent>,
    warnings: &mut Vec<String>,
    phrases: &mut Vec<Phrase>,
    phrase_src_start: &mut usize,
    phrase_time_start: &mut f32,
    opts: &CompileOptions<'_>,
) {
    let Token::Phoneme {
        code,
        stressed,
        pitch_delta,
        transient,
        span,
    } = tok
    else {
        return;
    };
    let phone_rate = if *stressed {
        state.rate * STRESS_DURATION_FACTOR
    } else {
        state.rate
    };
    render_phoneme_inner(
        code,
        *stressed,
        *pitch_delta,
        *transient,
        phone_rate,
        state,
        events,
        warnings,
        opts,
    );
    phrases.push(Phrase {
        src_start: *phrase_src_start,
        src_end: span.end,
        token_src_start: span.start,
        t_start_ms: *phrase_time_start,
        t_end_ms: state.time_ms,
        kind: PhraseKind::Phoneme,
        phoneme: Some(code.clone()),
    });
    *phrase_src_start = span.end;
    *phrase_time_start = state.time_ms;
    if !*transient {
        state.f0 += pitch_delta;
    }
}

#[allow(clippy::too_many_arguments)]
fn flush_syllable(
    state: &mut State,
    events: &mut Vec<MsEvent>,
    warnings: &mut Vec<String>,
    phrases: &mut Vec<Phrase>,
    phrase_src_start: &mut usize,
    phrase_time_start: &mut f32,
    queue: &mut Vec<Token>,
    opts: &CompileOptions<'_>,
) {
    if queue.is_empty() {
        return;
    }
    let slot = state.rate / queue.len() as f32;
    for tok in queue.drain(..) {
        let Token::Phoneme {
            code,
            stressed,
            pitch_delta,
            transient,
            span,
        } = &tok
        else {
            continue;
        };
        render_phoneme_inner(
            code,
            *stressed,
            *pitch_delta,
            *transient,
            slot,
            state,
            events,
            warnings,
            opts,
        );
        phrases.push(Phrase {
            src_start: *phrase_src_start,
            src_end: span.end,
            token_src_start: span.start,
            t_start_ms: *phrase_time_start,
            t_end_ms: state.time_ms,
            kind: PhraseKind::Phoneme,
            phoneme: Some(code.clone()),
        });
        *phrase_src_start = span.end;
        *phrase_time_start = state.time_ms;
        if !*transient {
            state.f0 += pitch_delta;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_phoneme_inner(
    code: &str,
    stressed: bool,
    pitch_delta: f32,
    _transient: bool,
    slot_ms: f32,
    state: &mut State,
    events: &mut Vec<MsEvent>,
    warnings: &mut Vec<String>,
    opts: &CompileOptions<'_>,
) {
    let p = match opts.phoneme_table.lookup(code) {
        Some(p) => p,
        None => {
            warnings.push(format!("unknown phoneme: {code}"));
            return;
        }
    };

    let start_f0 = if stressed {
        state.f0 + STRESS_F0_LIFT
    } else {
        state.f0
    };
    let end_f0 = start_f0 + pitch_delta;

    if p.is_stop {
        let burst_ms = STOP_BURST_MS.min(slot_ms * 0.3);
        let silence_ms = slot_ms - burst_ms;
        push_silence_with_transition(events, state, 20.0_f32.min(silence_ms * 0.4));
        state.time_ms += silence_ms;
        push_phoneme(
            events,
            state,
            &p,
            start_f0,
            None,
            5.0_f32.min(burst_ms * 0.2),
        );
        state.time_ms += burst_ms;
    } else if let Some(g) = p.glide_to {
        let onset = slot_ms * 0.25;
        let glide = slot_ms * 0.50;
        let offset = slot_ms * 0.25;
        push_phoneme(events, state, &p, start_f0, None, 20.0_f32.min(onset));
        state.time_ms += onset;
        push_phoneme(events, state, &p, end_f0, Some(g), glide);
        state.time_ms += glide + offset;
    } else if pitch_delta != 0.0 {
        push_phoneme(
            events,
            state,
            &p,
            start_f0,
            None,
            25.0_f32.min(slot_ms * 0.25),
        );
        state.time_ms += slot_ms * 0.25;
        push_phoneme(events, state, &p, end_f0, None, slot_ms * 0.6);
        state.time_ms += slot_ms * 0.75;
    } else {
        let trans = opts.default_transition_ms.min(slot_ms * 0.4);
        push_phoneme(events, state, &p, start_f0, None, trans);
        state.time_ms += slot_ms;
    }
}

fn push_phoneme(
    events: &mut Vec<MsEvent>,
    state: &State,
    p: &PhonemeParams,
    f0: f32,
    glide_to: Option<GlideTo>,
    transition_ms: f32,
) {
    let (f1, f2, f3) = match glide_to {
        Some(g) => (g.f1, g.f2, g.f3),
        None => (p.f1, p.f2, p.f3),
    };
    let target = ParamUpdate {
        f0: Some(f0),
        f1: Some(f1 * state.scale),
        f2: Some(f2 * state.scale),
        f3: Some(f3 * state.scale),
        bw1: Some(p.bw1 * state.scale),
        bw2: Some(p.bw2 * state.scale),
        bw3: Some(p.bw3 * state.scale),
        a1: Some(p.a1),
        a2: Some(p.a2),
        a3: Some(p.a3),
        voicing: Some(p.voicing),
        vibrato_depth: Some(state.extras.vibrato_depth),
        vibrato_rate: Some(state.extras.vibrato_rate),
        tremolo_depth: Some(state.extras.tremolo_depth),
        tremolo_rate: Some(state.extras.tremolo_rate),
        aspiration: Some(state.extras.aspiration),
        tilt: Some(state.extras.tilt),
        effort: Some(state.extras.effort),
        gain: None,
    };
    events.push(MsEvent::new(state.time_ms, target, transition_ms));
}

fn push_silence(events: &mut Vec<MsEvent>, state: &State) {
    push_silence_with_transition(events, state, SILENCE_TRANSITION_MS);
}

fn push_silence_with_transition(events: &mut Vec<MsEvent>, state: &State, transition_ms: f32) {
    let target = ParamUpdate {
        a1: Some(0.0),
        a2: Some(0.0),
        a3: Some(0.0),
        vibrato_depth: Some(state.extras.vibrato_depth),
        vibrato_rate: Some(state.extras.vibrato_rate),
        tremolo_depth: Some(state.extras.tremolo_depth),
        tremolo_rate: Some(state.extras.tremolo_rate),
        aspiration: Some(state.extras.aspiration),
        tilt: Some(state.extras.tilt),
        effort: Some(state.extras.effort),
        ..Default::default()
    };
    events.push(MsEvent::new(state.time_ms, target, transition_ms));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize::tokenize;

    #[test]
    fn empty_input_produces_only_trailing_silence() {
        let toks = tokenize("").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        // SENTENCE_FINAL_HOLD_MS (0) + TRAIL_OFF_MS (150). The FADE_OUT_MS is
        // the transition duration on the silence event, not elapsed time.
        assert!((r.total_ms - 150.0).abs() < 1e-3);
        // Just the fade-to-silence event
        assert_eq!(r.schedule.events().len(), 1);
    }

    #[test]
    fn hello_compiles_to_some_events() {
        let toks = tokenize("HH AH L OW").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        assert!(r.schedule.events().len() >= 4);
        assert!(r.warnings.is_empty());
        assert!(r.total_ms > 400.0); // 4 phonemes @ 110ms + tail
    }

    #[test]
    fn unknown_phoneme_warns() {
        let toks = tokenize("HH XYZZY OW").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("XYZZY"));
    }

    #[test]
    fn unmatched_close_paren_errors() {
        let toks = tokenize("AH ) AH").unwrap();
        let err = compile(&toks, &CompileOptions::default()).unwrap_err();
        assert!(matches!(err, CompileError::UnmatchedSyllableClose { .. }));
    }

    #[test]
    fn unclosed_open_paren_warns_and_flushes() {
        let toks = tokenize("AH ( HH AH").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("unclosed")));
        let phonemes: Vec<&str> = r
            .phrases
            .iter()
            .filter_map(|p| p.phoneme.as_deref())
            .collect();
        assert_eq!(phonemes, ["AH", "HH", "AH"]);
    }

    #[test]
    fn directive_changes_pitch() {
        let toks = tokenize("AH b220 AH").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        // The schedule should have a sequence where some events have f0=120 and
        // later some have f0=220
        let mut saw_220 = false;
        for e in r.schedule.events() {
            if let Some(f0) = e.target.f0 {
                if (f0 - 220.0).abs() < 0.01 {
                    saw_220 = true;
                }
            }
        }
        assert!(
            saw_220,
            "expected an event with f0=220 after the b220 directive"
        );
    }

    #[test]
    fn pitch_directive_relative() {
        let toks = tokenize("AH b+30 AH").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        let mut saw_150 = false;
        for e in r.schedule.events() {
            if let Some(f0) = e.target.f0 {
                if (f0 - 150.0).abs() < 0.01 {
                    saw_150 = true;
                }
            }
        }
        assert!(
            saw_150,
            "expected f0=150 after relative b+30 from default 120"
        );
    }

    #[test]
    fn syllable_groups_compress_to_one_slot() {
        // Single AH at default rate=110 should take longer than in (HH AH)
        // which compresses both into the same slot. Compare total_ms (excluding
        // the constant tail).
        let single = compile(&tokenize("AH").unwrap(), &CompileOptions::default()).unwrap();
        let group = compile(&tokenize("( HH AH )").unwrap(), &CompileOptions::default()).unwrap();
        // Both should be roughly the same duration (one rate-slot worth of time
        // + tail)
        assert!(
            (single.total_ms - group.total_ms).abs() < 1.0,
            "{} vs {}",
            single.total_ms,
            group.total_ms
        );
    }

    #[test]
    fn note_directive_sets_pitch() {
        let toks = tokenize("bA4 AH").unwrap();
        let r = compile(&toks, &CompileOptions::default()).unwrap();
        let mut saw_440 = false;
        for e in r.schedule.events() {
            if let Some(f0) = e.target.f0 {
                if (f0 - 440.0).abs() < 0.5 {
                    saw_440 = true;
                }
            }
        }
        assert!(saw_440, "expected f0~440 after bA4 directive");
    }
}
