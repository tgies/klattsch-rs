//! Render a phoneme string to a WAV file.
//!
//! ```sh
//! cargo run -p klattsch-wav --example render_to_wav -- "HH AH L OW" /tmp/hello.wav
//! ```

use std::process::ExitCode;

use klattsch_core::FormantSynth;
use klattsch_text::{compile_string, CompileOptions};
use klattsch_wav::{encode_wav, WavMetadata, WavOptions};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(text) = args.get(1) else {
        eprintln!("usage: render_to_wav <phoneme-string> [output.wav]");
        eprintln!("  e.g. render_to_wav \"HH AH L OW\" hello.wav");
        return ExitCode::from(2);
    };
    let out_path: &str = args.get(2).map_or("klattsch.wav", String::as_str);

    let opts = CompileOptions::default();
    let result = match compile_string(text, &opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("compile error: {e}");
            return ExitCode::from(1);
        }
    };
    if !result.warnings.is_empty() {
        eprintln!("warnings: {}", result.warnings.join(", "));
    }

    let total_samples =
        ((result.total_ms * opts.sample_rate as f32 / 1000.0).ceil() as usize).max(1);
    let mut buf = vec![0.0f32; total_samples];
    let mut synth = FormantSynth::new(opts.sample_rate);
    synth.queue_schedule(result.schedule);
    synth.process(&mut buf);

    let wav = encode_wav(
        &buf,
        opts.sample_rate,
        &WavOptions {
            peak_normalize: 0.95,
            metadata: Some(WavMetadata {
                software: Some("klattsch-rs"),
                comment: Some(text),
            }),
        },
    );

    if let Err(e) = std::fs::write(out_path, &wav.bytes) {
        eprintln!("could not write {out_path}: {e}");
        return ExitCode::from(1);
    }

    eprintln!(
        "wrote {out_path}: {} KB, {:.2}s, normalize gain {:.2}x",
        wav.bytes.len() / 1024,
        result.total_ms / 1000.0,
        wav.gain
    );
    ExitCode::SUCCESS
}
