//! Realtime-safe parallel-formant speech synthesis engine.
//!
//! [`FormantSynth::process`] does not allocate, take locks, or perform I/O.
//! Construction, [`FormantSynth::reset`], and [`FormantSynth::queue_schedule`]
//! allocate; call those off the audio thread.

pub mod banks;
pub mod dsp;
pub mod params;
pub mod phonemes;
pub mod schedule;
pub mod synth;

#[cfg(feature = "live-events")]
pub mod live;

pub use banks::{Bank, BankError, BankRegistry, BankSource, PhonemeEdit, PhonemeEntry};
pub use params::{ParamUpdate, Params};
pub use phonemes::{Arpabet, GlideTo, PhonemeParams, PhonemeTable, ARPABET};
pub use schedule::{MsEvent, Schedule, ScheduleEvent};
pub use synth::FormantSynth;
