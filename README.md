# klattsch-rs

Rust port of [klattsch](https://github.com/tgies/klattsch), a parallel-formant
(Klatt 1980) speech synthesizer.

## Crates

| Crate           | Purpose                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------ |
| `klattsch-core` | Synthesis engine. `FormantSynth::process` is realtime-safe. Single voice.                                          |
| `klattsch-text` | Parser and schedule compiler for `.klatt` phoneme strings.                                                         |
| `klattsch-wav`  | RIFF/WAVE encoder for rendered mono PCM.                                                                           |
| `klattsch-wasm` | wasm-bindgen build with AudioWorklet shim. See [`crates/klattsch-wasm/README.md`](crates/klattsch-wasm/README.md). |

## Versioning

`klattsch-rs` uses its own SemVer track, independent from the JavaScript
`klattsch` package. The Rust crates expose Rust-native APIs rather than a
drop-in replacement for the JS package API, so their version numbers do not
mirror JS releases.

For now, the public Rust crates are versioned together. Patch releases are for
bug fixes and compatible additions; minor releases may change Rust APIs while
the project is still `0.x`.

This release is parity-tested against `klattsch` JS `0.3.0`.

Minimum supported Rust version: `1.77`.

## Usage

```rust
use klattsch_core::FormantSynth;
use klattsch_text::{compile_string, CompileOptions};

let opts = CompileOptions::default();
let result = compile_string("HH AH L OW", &opts).unwrap();
let total = (result.total_ms * opts.sample_rate as f32 / 1000.0).ceil() as usize;
let mut buf = vec![0.0f32; total];
let mut synth = FormantSynth::new(opts.sample_rate);
synth.queue_schedule(result.schedule);
synth.process(&mut buf);
```

```sh
cargo run -p klattsch-wav --example render_to_wav -- "HH AH L OW" /tmp/hello.wav
```

## Live parameter control

Behind the `live-events` feature:

```rust
use klattsch_core::{FormantSynth, ParamUpdate};
use klattsch_core::live::{event_channel, SynthEvent};

let mut synth = FormantSynth::new(48_000);
let (mut tx, mut rx) = event_channel(256);

tx.try_send(SynthEvent::SetTarget {
    target: ParamUpdate { f0: Some(220.0), ..Default::default() },
    transition_samples: 480,
}).ok();

let mut buf = [0.0f32; 256];
synth.drain_events(&mut rx);
synth.process(&mut buf);
```

`FormantSynth::process` and `FormantSynth::drain_events` do not allocate, take
locks, or perform I/O. `new`, `reset`, and `queue_schedule` allocate.

## Tests

```sh
cargo test --workspace --all-features
```

`klattsch-text/tests/parity.rs` diffs Rust output against committed JS-rendered
reference WAVs in `tests/golden/`.
Regenerate with:

```sh
npm run goldens
```

## License

MIT
