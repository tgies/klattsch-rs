//! Diagnostic for the stress parity divergence. Prints JS vs Rust samples
//! around the largest error so we can see whether it's a phase shift,
//! amplitude shift, or structural difference.

use std::fs;
use std::path::{Path, PathBuf};

use klattsch_core::FormantSynth;
use klattsch_text::{compile_string, CompileOptions};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
}

fn read_wav_samples_f32(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).unwrap();
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            let data_start = offset + 8;
            return bytes[data_start..data_start + size]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes(c.try_into().unwrap()) as f32 / 32767.0)
                .collect();
        }
        offset += 8 + size + (size & 1);
    }
    panic!()
}

fn render_via_rust(text: &str, sample_rate: u32) -> Vec<f32> {
    let opts = CompileOptions {
        sample_rate,
        ..Default::default()
    };
    let r = compile_string(text, &opts).unwrap();
    let total = ((r.total_ms * sample_rate as f32 / 1000.0).ceil() as usize).max(1);
    let mut buf = vec![0.0f32; total];
    let mut s = FormantSynth::new(sample_rate);
    s.queue_schedule(r.schedule);
    s.process(&mut buf);
    buf
}

fn quantize(v: f32) -> f32 {
    let q = (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
    q as f32 / 32767.0
}

/// Diagnostic, not an assertion. Run explicitly when investigating divergence:
///
/// ```sh
/// cargo test -p klattsch-text --test parity_diag -- --ignored --nocapture
/// ```
#[test]
#[ignore = "diagnostic; run with --ignored"]
fn diagnose_stress_divergence() {
    let dir = golden_dir();
    let js = read_wav_samples_f32(&dir.join("stress.wav"));
    let rust_raw = render_via_rust("AH ! AH", 48_000);
    let rust: Vec<f32> = rust_raw.iter().map(|v| quantize(*v)).collect();

    eprintln!("rust_len={}, js_len={}", rust.len(), js.len());

    // Find the first sample where they diverge by more than 1e-3
    let n = rust.len().min(js.len());
    let mut first_div: Option<usize> = None;
    for i in 0..n {
        if (rust[i] - js[i]).abs() > 1e-3 {
            first_div = Some(i);
            break;
        }
    }
    eprintln!("first divergence (>1e-3): {first_div:?}");

    if let Some(i) = first_div {
        let lo = i.saturating_sub(8);
        let hi = (i + 16).min(n);
        eprintln!("  idx     |    rust       |    js         | diff");
        eprintln!("  --------|---------------|---------------|--------");
        for j in lo..hi {
            let mark = if j == i { " *" } else { "  " };
            eprintln!(
                "  {:>6}{} | {:>13.6} | {:>13.6} | {:>8.5}",
                j,
                mark,
                rust[j],
                js[j],
                rust[j] - js[j]
            );
        }
    }

    // Also: check if Rust output equals JS output shifted by N samples
    // (phase lag)
    for shift in -3i32..=3 {
        let mut max_err = 0.0f32;
        for (i, r) in rust.iter().enumerate().skip(100).take(n - 200) {
            let j = (i as i32 + shift) as usize;
            if j < n {
                let e = (r - js[j]).abs();
                if e > max_err {
                    max_err = e;
                }
            }
        }
        eprintln!("shift={shift:+}: max_err={max_err:.4e}");
    }
}
