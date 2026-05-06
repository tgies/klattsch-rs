//! Parity tests against JS-rendered reference WAVs in `tests/golden/`.
//!
//! Goldens are produced by `tools/render-goldens.mjs` running the JavaScript
//! klattsch engine. This test renders the same inputs through Rust, quantizes
//! through the same i16 WAV round-trip, and asserts per-sample absolute error
//! stays below `TOLERANCE`. If a parity test fails, either:
//!   1. the Rust port has diverged from JS, or
//!   2. the JS engine changed and the goldens need regenerating
//!      (run `node tools/render-goldens.mjs` if so).

use std::fs;
use std::path::{Path, PathBuf};

use klattsch_core::FormantSynth;
use klattsch_text::{compile_string, CompileOptions};
use serde::Deserialize;

/// Workspace-relative location of the committed JS reference WAVs.
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
}

/// Per-sample absolute error tolerance. Accounts for:
/// - i16 WAV quantization noise (~3e-5)
/// - V8 Math.sin/cos vs Rust libm divergence (~1e-7 per call)
/// - Accumulated divergence in the IIR biquad chain (~1e-3 over
///   thousands of samples)
///
/// The engine uses f64 internally for phases, biquad state, and
/// glottal_pulse to match JS Number precision. The remaining divergence
/// is purely from different libm implementations of sin/cos.
const TOLERANCE: f32 = 1.5e-3;

#[derive(Deserialize)]
struct ManifestEntry {
    name: String,
    text: String,
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
}

fn read_wav_samples_f32(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(&bytes[0..4], b"RIFF", "{}: not a RIFF file", path.display());
    assert_eq!(&bytes[8..12], b"WAVE", "{}: not WAVE", path.display());

    // Walk chunks after the WAVE fourcc looking for `data`.
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            let data_start = offset + 8;
            let data_end = data_start + size;
            return bytes[data_start..data_end]
                .chunks_exact(2)
                .map(|c| {
                    let v = i16::from_le_bytes(c.try_into().unwrap());
                    v as f32 / 32767.0
                })
                .collect();
        }
        // Chunks are word-aligned: skip header + size + 1 pad byte if size is odd.
        offset += 8 + size + (size & 1);
    }
    panic!("no data chunk in {}", path.display());
}

fn render_via_rust(text: &str, sample_rate: u32) -> Vec<f32> {
    let opts = CompileOptions {
        sample_rate,
        ..Default::default()
    };
    let r = compile_string(text, &opts).expect("compile");
    let total = ((r.total_ms * sample_rate as f32 / 1000.0).ceil() as usize).max(1);
    let mut buf = vec![0.0f32; total];
    let mut s = FormantSynth::new(sample_rate);
    s.queue_schedule(r.schedule);
    s.process(&mut buf);
    buf
}

/// Round-trip a sample through the same i16 quantization the WAV writer uses,
/// matching what the JS golden went through.
fn quantize(v: f32) -> f32 {
    let q = (v.clamp(-1.0, 1.0) * 32767.0).round() as i32;
    let q = q.clamp(-32768, 32767) as i16;
    q as f32 / 32767.0
}

#[test]
fn parity_with_js_goldens() {
    let dir = golden_dir();
    let manifest_path = dir.join("manifest.json");
    let json = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("read manifest {}: {e}", manifest_path.display()));
    let entries: Vec<ManifestEntry> = serde_json::from_str(&json).expect("parse manifest");

    let mut failures: Vec<String> = Vec::new();

    for entry in &entries {
        let wav_path = dir.join(format!("{}.wav", entry.name));
        let js_samples = read_wav_samples_f32(&wav_path);
        let rust_raw = render_via_rust(&entry.text, entry.sample_rate);
        let rust_samples: Vec<f32> = rust_raw.iter().map(|v| quantize(*v)).collect();

        // Allow off-by-1 in length (different ceiling rounding in float at
        // boundary).
        let len_diff =
            (rust_samples.len() as i64 - js_samples.len() as i64).unsigned_abs() as usize;
        if len_diff > 1 {
            failures.push(format!(
                "{}: length mismatch (rust {} vs js {})",
                entry.name,
                rust_samples.len(),
                js_samples.len()
            ));
            continue;
        }

        let n = rust_samples.len().min(js_samples.len());
        let mut max_err = 0.0f32;
        let mut max_err_idx = 0usize;
        let mut diff_count = 0usize;
        for i in 0..n {
            let err = (rust_samples[i] - js_samples[i]).abs();
            if err > 0.0 {
                diff_count += 1;
            }
            if err > max_err {
                max_err = err;
                max_err_idx = i;
            }
        }

        if max_err > TOLERANCE {
            failures.push(format!(
                "{}: max abs error {:.6e} > {:.0e} at sample {}/{} ({} samples differ)",
                entry.name, max_err, TOLERANCE, max_err_idx, n, diff_count
            ));
        } else {
            eprintln!(
                "  {} OK  (max err {:.2e}, {} samples differ of {})",
                entry.name, max_err, diff_count, n
            );
        }
    }

    if !failures.is_empty() {
        panic!("parity failures:\n  {}", failures.join("\n  "));
    }
}
