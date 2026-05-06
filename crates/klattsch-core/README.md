# klattsch-core

Realtime-safe parallel-formant speech synthesis engine for `klattsch-rs`.

`klattsch-core` contains the sample generator, synthesis parameters, phoneme
tables, schedules, and optional live event channel. `FormantSynth::process` and
`FormantSynth::drain_events` do not allocate, take locks, or perform I/O.

See the workspace README for examples and release notes:
https://github.com/tgies/klattsch-rs
