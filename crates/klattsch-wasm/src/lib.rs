//! WebAssembly build. Built with `wasm-pack build --target web`. See
//! `crates/klattsch-wasm/README.md` for usage.

use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct FormantSynth {
    inner: klattsch_core::FormantSynth,
}

#[wasm_bindgen]
impl FormantSynth {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            inner: klattsch_core::FormantSynth::new(sample_rate),
        }
    }

    /// Explicit noise LFSR seed for decorrelating noise across voices.
    #[wasm_bindgen(js_name = withSeed)]
    pub fn with_seed(sample_rate: u32, noise_seed: u32) -> Self {
        Self {
            inner: klattsch_core::FormantSynth::with_seed(sample_rate, noise_seed),
        }
    }

    /// Render `out.length` samples of mono f32 PCM in `[-1, 1]`.
    pub fn process(&mut self, out: &mut [f32]) {
        self.inner.process(out);
    }

    #[wasm_bindgen(js_name = queueSchedule)]
    pub fn queue_schedule(&mut self, schedule: Schedule) {
        self.inner.queue_schedule(schedule.inner);
    }

    /// Build a schedule from a JS array of `{atMs, target, transitionMs}` and
    /// queue it. Mirrors the original engine's `{type: 'schedule'}` message.
    #[wasm_bindgen(js_name = queueScheduleFromMs)]
    pub fn queue_schedule_from_ms(&mut self, events: JsValue) -> Result<(), JsError> {
        let parsed: Vec<WasmMsEvent> =
            serde_wasm_bindgen::from_value(events).map_err(|e| JsError::new(&e.to_string()))?;
        let sr = self.inner.sample_rate();
        let ms_events = parsed.into_iter().map(|e| {
            klattsch_core::MsEvent::new(e.at_ms, e.target.into(), e.transition_ms.unwrap_or(30.0))
        });
        let sched = klattsch_core::Schedule::from_ms_events(sr, ms_events);
        self.inner.queue_schedule(sched);
        Ok(())
    }

    /// Stage a sparse parameter update with a `transitionMs`-long linear ramp.
    /// `target` is a plain JS object using the JS engine's PARAMS field names
    /// (`F0`, `voicing`, `F1`, `BW1`, `A1`, ..., `vibratoDepth`, ...).
    #[wasm_bindgen(js_name = setTarget)]
    pub fn set_target(&mut self, target: JsValue, transition_ms: f32) -> Result<(), JsError> {
        let upd: WasmParamUpdate =
            serde_wasm_bindgen::from_value(target).map_err(|e| JsError::new(&e.to_string()))?;
        let sr = self.inner.sample_rate() as f32;
        let n = ((transition_ms * sr / 1000.0).floor() as i64).max(1) as u32;
        self.inner.set_target(upd.into(), n);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.inner.reset(klattsch_core::Params::DEFAULT);
    }

    #[wasm_bindgen(getter, js_name = sampleRate)]
    pub fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
}

/// Opaque handle to a compiled schedule.
#[wasm_bindgen]
pub struct Schedule {
    inner: klattsch_core::Schedule,
}

#[wasm_bindgen]
pub struct CompileResult {
    schedule: Option<klattsch_core::Schedule>,
    total_ms: f32,
    warnings: Vec<String>,
}

#[wasm_bindgen]
impl CompileResult {
    /// Total utterance duration in ms, including the trailing fade.
    #[wasm_bindgen(getter, js_name = totalMs)]
    pub fn total_ms(&self) -> f32 {
        self.total_ms
    }

    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> Vec<JsValue> {
        self.warnings.iter().map(JsValue::from).collect()
    }

    /// Move the schedule out. Subsequent calls return `undefined`.
    #[wasm_bindgen(js_name = takeSchedule)]
    pub fn take_schedule(&mut self) -> Option<Schedule> {
        self.schedule.take().map(|inner| Schedule { inner })
    }
}

/// Compile a phoneme string at 48 kHz.
#[wasm_bindgen(js_name = compileString)]
pub fn compile_string(text: &str) -> Result<CompileResult, JsError> {
    compile_string_at(text, 48_000)
}

/// Compile a phoneme string at the given sample rate.
#[wasm_bindgen(js_name = compileStringAt)]
pub fn compile_string_at(text: &str, sample_rate: u32) -> Result<CompileResult, JsError> {
    let opts = klattsch_text::CompileOptions {
        sample_rate,
        ..Default::default()
    };
    let r = klattsch_text::compile_string(text, &opts).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(CompileResult {
        schedule: Some(r.schedule),
        total_ms: r.total_ms,
        warnings: r.warnings,
    })
}

/// Encode mono f32 PCM as a 16-bit RIFF/WAVE blob.
#[wasm_bindgen(js_name = encodeWav)]
pub fn encode_wav(samples: &[f32], sample_rate: u32, peak_normalize: f32) -> Vec<u8> {
    let opts = klattsch_wav::WavOptions {
        peak_normalize,
        metadata: None,
    };
    klattsch_wav::encode_wav(samples, sample_rate, &opts).bytes
}

// JS-side parameter update. Field names match the JS engine's PARAMS array
// (`F0`, `BW1`, `vibratoDepth`, ...) so callers can pass the same target
// objects they used with the original AudioWorklet.
#[derive(Default, Deserialize)]
#[serde(default)]
struct WasmParamUpdate {
    #[serde(rename = "F0")]
    f0: Option<f32>,
    voicing: Option<f32>,
    #[serde(rename = "F1")]
    f1: Option<f32>,
    #[serde(rename = "BW1")]
    bw1: Option<f32>,
    #[serde(rename = "A1")]
    a1: Option<f32>,
    #[serde(rename = "F2")]
    f2: Option<f32>,
    #[serde(rename = "BW2")]
    bw2: Option<f32>,
    #[serde(rename = "A2")]
    a2: Option<f32>,
    #[serde(rename = "F3")]
    f3: Option<f32>,
    #[serde(rename = "BW3")]
    bw3: Option<f32>,
    #[serde(rename = "A3")]
    a3: Option<f32>,
    gain: Option<f32>,
    #[serde(rename = "vibratoDepth")]
    vibrato_depth: Option<f32>,
    #[serde(rename = "vibratoRate")]
    vibrato_rate: Option<f32>,
    #[serde(rename = "tremoloDepth")]
    tremolo_depth: Option<f32>,
    #[serde(rename = "tremoloRate")]
    tremolo_rate: Option<f32>,
    aspiration: Option<f32>,
    tilt: Option<f32>,
    effort: Option<f32>,
}

impl From<WasmParamUpdate> for klattsch_core::ParamUpdate {
    fn from(w: WasmParamUpdate) -> Self {
        klattsch_core::ParamUpdate {
            f0: w.f0,
            voicing: w.voicing,
            f1: w.f1,
            bw1: w.bw1,
            a1: w.a1,
            f2: w.f2,
            bw2: w.bw2,
            a2: w.a2,
            f3: w.f3,
            bw3: w.bw3,
            a3: w.a3,
            gain: w.gain,
            vibrato_depth: w.vibrato_depth,
            vibrato_rate: w.vibrato_rate,
            tremolo_depth: w.tremolo_depth,
            tremolo_rate: w.tremolo_rate,
            aspiration: w.aspiration,
            tilt: w.tilt,
            effort: w.effort,
        }
    }
}

#[derive(Deserialize)]
struct WasmMsEvent {
    #[serde(rename = "atMs")]
    at_ms: f32,
    target: WasmParamUpdate,
    #[serde(rename = "transitionMs")]
    transition_ms: Option<f32>,
}
