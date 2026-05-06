//! Time-stamped parameter event schedule consumed by [`FormantSynth`](crate::FormantSynth).

use crate::params::ParamUpdate;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// One entry in a [`Schedule`]: at sample `at_sample`, begin a linear ramp of
/// `transition_samples` samples from the synth's current state toward the
/// fields specified in `target`. Fields not present in `target` keep their
/// previous target value (which may itself still be ramping).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ScheduleEvent {
    /// Absolute sample index at which this event fires.
    pub at_sample: u64,
    /// Sparse parameter overrides.
    pub target: ParamUpdate,
    /// Number of samples over which to linearly ramp from current to target.
    /// Always at least 1 in practice; the synth clamps to >= 1.
    pub transition_samples: u32,
}

/// Time-sorted schedule of parameter events bound to a specific sample rate.
///
/// Construct via [`Schedule::from_sample_events`], [`Schedule::from_ms_events`],
/// or [`Schedule::empty`]. Construction allocates; the synth's `process`
/// function never does.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Schedule {
    pub(crate) events: Box<[ScheduleEvent]>,
    /// Sample rate the events were quantized at. The consuming `FormantSynth`
    /// must run at this rate (we don't resample on the fly).
    pub sample_rate: u32,
}

impl Schedule {
    /// Empty schedule at the given sample rate.
    pub fn empty(sample_rate: u32) -> Self {
        Self {
            events: Box::new([]),
            sample_rate,
        }
    }

    /// Construct from events already in samples. Sorts by `at_sample`.
    pub fn from_sample_events(sample_rate: u32, mut events: Vec<ScheduleEvent>) -> Self {
        events.sort_by_key(|e| e.at_sample);
        Self {
            events: events.into_boxed_slice(),
            sample_rate,
        }
    }

    /// Build from millisecond-keyed events. `at_sample = floor(ms * sr / 1000)`;
    /// `transition_samples` is clamped to >= 1.
    pub fn from_ms_events<I>(sample_rate: u32, ms_events: I) -> Self
    where
        I: IntoIterator<Item = MsEvent>,
    {
        let sr_f = sample_rate as f32;
        let events: Vec<ScheduleEvent> = ms_events
            .into_iter()
            .map(|m| ScheduleEvent {
                at_sample: (m.at_ms * sr_f / 1000.0).floor().max(0.0) as u64,
                target: m.target,
                transition_samples: ((m.transition_ms * sr_f / 1000.0).floor() as i64).max(1)
                    as u32,
            })
            .collect();
        Self::from_sample_events(sample_rate, events)
    }

    /// Read-only access to the events.
    pub fn events(&self) -> &[ScheduleEvent] {
        &self.events
    }

    /// Sample index of the last event, or 0 if empty.
    pub fn last_event_sample(&self) -> u64 {
        self.events.last().map_or(0, |e| e.at_sample)
    }
}

/// Millisecond-keyed event, the natural shape for hand-authored schedules.
/// Convert to [`Schedule`] via [`Schedule::from_ms_events`].
#[derive(Clone, Copy, Debug)]
pub struct MsEvent {
    pub at_ms: f32,
    pub target: ParamUpdate,
    pub transition_ms: f32,
}

impl MsEvent {
    pub const fn new(at_ms: f32, target: ParamUpdate, transition_ms: f32) -> Self {
        Self {
            at_ms,
            target,
            transition_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_schedule_has_no_events() {
        let s = Schedule::empty(48_000);
        assert_eq!(s.events().len(), 0);
        assert_eq!(s.last_event_sample(), 0);
    }

    #[test]
    fn from_ms_converts_and_sorts() {
        let s = Schedule::from_ms_events(
            48_000,
            [
                MsEvent::new(
                    100.0,
                    ParamUpdate {
                        f0: Some(220.0),
                        ..Default::default()
                    },
                    30.0,
                ),
                MsEvent::new(
                    0.0,
                    ParamUpdate {
                        f0: Some(120.0),
                        ..Default::default()
                    },
                    35.0,
                ),
            ],
        );
        // Sorted ascending
        assert_eq!(s.events()[0].at_sample, 0);
        // 100 ms at 48000 Hz = 4800 samples
        assert_eq!(s.events()[1].at_sample, 4800);
        // 35 ms at 48000 = 1680 samples
        assert_eq!(s.events()[0].transition_samples, 1680);
    }

    #[test]
    fn ms_transition_clamps_to_at_least_one_sample() {
        let s = Schedule::from_ms_events(48_000, [MsEvent::new(0.0, ParamUpdate::default(), 0.0)]);
        assert_eq!(s.events()[0].transition_samples, 1);
    }
}
