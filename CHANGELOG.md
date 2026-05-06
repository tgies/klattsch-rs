# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-05-06

### Added

- `klattsch-core`: realtime-safe `FormantSynth`.
- `klattsch-core`: `live-events` feature with an SPSC ring channel for
  thread-safe parameter updates.
- `klattsch-text`: phoneme-string parser and sequencer.
- `klattsch-text`: parity test harness diffing Rust output against JS-rendered
  reference WAVs.
- `klattsch-wav`: RIFF/WAVE encoder for rendered mono PCM.
- `klattsch-wasm`: wasm-bindgen build with `setTarget`, `queueScheduleFromMs`,
  and an AudioWorklet shim that mirrors the original engine's `frame` /
  `schedule` / `compile` / `reset` message protocol.
