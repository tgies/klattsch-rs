//! [`FormantSynth`] - the realtime-safe synthesis engine.

use crate::dsp::{glottal_pulse, soft_clip, BandpassBiquad, Xorshift32};
use crate::params::{ParamUpdate, Params};
use crate::schedule::{Schedule, ScheduleEvent};

const ZERO_INCREMENTS: Params = Params {
    f0: 0.0,
    voicing: 0.0,
    f1: 0.0,
    bw1: 0.0,
    a1: 0.0,
    f2: 0.0,
    bw2: 0.0,
    a2: 0.0,
    f3: 0.0,
    bw3: 0.0,
    a3: 0.0,
    gain: 0.0,
    vibrato_depth: 0.0,
    vibrato_rate: 0.0,
    tremolo_depth: 0.0,
    tremolo_rate: 0.0,
    aspiration: 0.0,
    tilt: 0.0,
    effort: 0.0,
};

macro_rules! for_each_param {
    ($action:ident) => {
        $action!(f0);
        $action!(voicing);
        $action!(f1);
        $action!(bw1);
        $action!(a1);
        $action!(f2);
        $action!(bw2);
        $action!(a2);
        $action!(f3);
        $action!(bw3);
        $action!(a3);
        $action!(gain);
        $action!(vibrato_depth);
        $action!(vibrato_rate);
        $action!(tremolo_depth);
        $action!(tremolo_rate);
        $action!(aspiration);
        $action!(tilt);
        $action!(effort);
    };
}

/// Klatt-style parallel-formant speech synth.
pub struct FormantSynth {
    sr: f32,

    current: Params,
    target: Params,
    increment: Params,
    transition_samples: u32,

    glottal_phase: f64,
    vibrato_phase: f64,
    tremolo_phase: f64,
    tilt_prev: f32,

    bp1: BandpassBiquad,
    bp2: BandpassBiquad,
    bp3: BandpassBiquad,

    noise: Xorshift32,
    noise_seed: u32,

    schedule: Option<Schedule>,
    schedule_idx: usize,
    sample_clock: u64,
}

impl FormantSynth {
    /// Construct at `sample_rate` with the JS-equivalent default noise seed.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_seed(sample_rate, Xorshift32::DEFAULT_SEED)
    }

    /// Construct with an explicit noise LFSR seed. Use distinct seeds per
    /// voice when running multiple synths in parallel so their unvoiced noise
    /// is uncorrelated.
    pub fn with_seed(sample_rate: u32, noise_seed: u32) -> Self {
        assert!(sample_rate > 0, "sample_rate must be positive");
        Self {
            sr: sample_rate as f32,
            current: Params::DEFAULT,
            target: Params::DEFAULT,
            increment: ZERO_INCREMENTS,
            transition_samples: 0,
            glottal_phase: 0.0_f64,
            vibrato_phase: 0.0_f64,
            tremolo_phase: 0.0_f64,
            tilt_prev: 0.0,
            bp1: BandpassBiquad::new(),
            bp2: BandpassBiquad::new(),
            bp3: BandpassBiquad::new(),
            noise: Xorshift32::with_seed(noise_seed),
            noise_seed,
            schedule: None,
            schedule_idx: 0,
            sample_clock: 0,
        }
    }

    /// Sample rate the synth was constructed at.
    pub fn sample_rate(&self) -> u32 {
        self.sr as u32
    }

    /// Snapshot of the current (post-ramp) parameter state.
    pub fn current(&self) -> Params {
        self.current
    }

    /// Reset all internal state. `initial` becomes both current and target;
    /// the noise LFSR returns to its constructed seed; the schedule is cleared.
    pub fn reset(&mut self, initial: Params) {
        self.current = initial;
        self.target = initial;
        self.increment = ZERO_INCREMENTS;
        self.transition_samples = 0;
        self.glottal_phase = 0.0;
        self.vibrato_phase = 0.0;
        self.tremolo_phase = 0.0;
        self.tilt_prev = 0.0;
        self.bp1.reset();
        self.bp2.reset();
        self.bp3.reset();
        self.noise.reset(self.noise_seed);
        self.schedule = None;
        self.schedule_idx = 0;
        self.sample_clock = 0;
    }

    /// Replace any pending schedule and rewind the sample clock to 0.
    ///
    /// # Panics
    /// If `schedule.sample_rate` does not match this synth's sample rate.
    pub fn queue_schedule(&mut self, schedule: Schedule) {
        assert_eq!(
            schedule.sample_rate, self.sr as u32,
            "Schedule sample_rate must match FormantSynth sample_rate"
        );
        self.schedule = Some(schedule);
        self.schedule_idx = 0;
        self.sample_clock = 0;
    }

    /// Stage a sparse parameter update with a `transition_samples`-long linear
    /// ramp from current values.
    pub fn set_target(&mut self, update: ParamUpdate, transition_samples: u32) {
        self.transition_samples = transition_samples.max(1);
        update.apply_to(&mut self.target);
        self.recompute_increments();
    }

    fn recompute_increments(&mut self) {
        let n = self.transition_samples.max(1) as f32;
        macro_rules! delta {
            ($f:ident) => {
                self.increment.$f = (self.target.$f - self.current.$f) / n;
            };
        }
        for_each_param!(delta);
    }

    // Returns by value so the borrow on `self.schedule` ends before the
    // caller mutates other `self` fields.
    fn pop_due_event(&mut self) -> Option<ScheduleEvent> {
        let s = self.schedule.as_ref()?;
        let evt = *s.events.get(self.schedule_idx)?;
        if evt.at_sample <= self.sample_clock {
            self.schedule_idx += 1;
            Some(evt)
        } else {
            None
        }
    }

    /// Render `out.len()` samples of mono f32 PCM in `[-1.0, 1.0]`.
    pub fn process(&mut self, out: &mut [f32]) {
        const TAU64: f64 = 2.0 * core::f64::consts::PI;
        let sr64 = self.sr as f64;

        for sample in out.iter_mut() {
            while let Some(evt) = self.pop_due_event() {
                self.transition_samples = evt.transition_samples.max(1);
                evt.target.apply_to(&mut self.target);
                self.recompute_increments();
            }
            self.sample_clock += 1;

            if self.transition_samples > 0 {
                macro_rules! step {
                    ($f:ident) => {
                        self.current.$f += self.increment.$f;
                    };
                }
                for_each_param!(step);
                self.transition_samples -= 1;
                if self.transition_samples == 0 {
                    self.current = self.target;
                }
            }

            self.vibrato_phase += TAU64 * self.current.vibrato_rate as f64 / sr64;
            self.vibrato_phase -= TAU64 * (self.vibrato_phase / TAU64).floor();
            let eff_f0 =
                self.current.f0 + self.current.vibrato_depth * self.vibrato_phase.sin() as f32;

            self.tremolo_phase += TAU64 * self.current.tremolo_rate as f64 / sr64;
            self.tremolo_phase -= TAU64 * (self.tremolo_phase / TAU64).floor();
            let tremolo_mod =
                1.0 - self.current.tremolo_depth * (0.5 + 0.5 * self.tremolo_phase.sin() as f32);

            let v = self.current.voicing.clamp(0.0, 1.0);
            let noise_sample = self.noise.next_sample();
            let pulse_val = glottal_pulse(self.glottal_phase, self.current.effort);
            let voiced_gain = 1.0 - self.current.aspiration * 0.85;
            let exc = v * pulse_val * voiced_gain
                + (1.0 - v) * noise_sample * 0.35
                + self.current.aspiration * noise_sample * 0.5;
            self.glottal_phase += eff_f0 as f64 / sr64;
            self.glottal_phase -= self.glottal_phase.floor();

            self.bp1
                .set_freq(self.current.f1, self.current.bw1, self.sr);
            self.bp2
                .set_freq(self.current.f2, self.current.bw2, self.sr);
            self.bp3
                .set_freq(self.current.f3, self.current.bw3, self.sr);

            let y = (self.bp1.process(exc) * self.current.a1
                + self.bp2.process(exc) * self.current.a2
                + self.bp3.process(exc) * self.current.a3)
                * self.current.gain
                * tremolo_mod;

            let tilted = y - self.current.tilt * self.tilt_prev;
            self.tilt_prev = y;
            *sample = soft_clip(tilted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vowel_target() -> ParamUpdate {
        ParamUpdate {
            f0: Some(220.0),
            voicing: Some(1.0),
            a1: Some(1.0),
            a2: Some(0.9),
            a3: Some(0.7),
            ..Default::default()
        }
    }

    #[test]
    fn default_state_is_silence() {
        // DEFAULT has voicing=0 and amplitudes=0, so the formant outputs are
        // multiplied by zero -> zero output.
        let mut s = FormantSynth::new(48_000);
        let mut buf = [0.0f32; 256];
        s.process(&mut buf);
        for v in &buf {
            assert!(v.abs() < 1e-6, "expected silence, got {v}");
        }
    }

    #[test]
    fn vowel_produces_audio() {
        let mut s = FormantSynth::new(48_000);
        s.set_target(vowel_target(), 1);
        let mut buf = [0.0f32; 4800]; // 100 ms
        s.process(&mut buf);
        let peak = buf.iter().fold(0.0f32, |p, v| p.max(v.abs()));
        assert!(peak > 0.05, "expected audible output, peak {peak}");
    }

    #[test]
    fn block_size_does_not_change_output() {
        // Render 2400 samples in one call vs. 24 calls of 100. Sample-identical.
        let mut s_full = FormantSynth::new(48_000);
        s_full.set_target(vowel_target(), 1);
        let mut buf_full = [0.0f32; 2400];
        s_full.process(&mut buf_full);

        let mut s_chunked = FormantSynth::new(48_000);
        s_chunked.set_target(vowel_target(), 1);
        let mut buf_chunked = [0.0f32; 2400];
        for chunk in buf_chunked.chunks_mut(100) {
            s_chunked.process(chunk);
        }

        for i in 0..2400 {
            assert!(
                (buf_full[i] - buf_chunked[i]).abs() < 1e-6,
                "block-size divergence at sample {i}: {} vs {}",
                buf_full[i],
                buf_chunked[i]
            );
        }
    }

    #[test]
    fn schedule_event_changes_amplitude() {
        // Schedule: silence for 100ms, then vowel for 100ms. Output amplitude
        // should be ~0 in the first half and audible in the second half.
        let sr = 48_000u32;
        let sched = Schedule::from_ms_events(
            sr,
            [
                crate::schedule::MsEvent::new(0.0, ParamUpdate::default(), 1.0),
                crate::schedule::MsEvent::new(100.0, vowel_target(), 1.0),
            ],
        );
        let mut s = FormantSynth::new(sr);
        s.queue_schedule(sched);
        let mut buf = [0.0f32; 9600]; // 200 ms
        s.process(&mut buf);

        let first_half_peak = buf[..4800].iter().fold(0.0f32, |p, v| p.max(v.abs()));
        let second_half_peak = buf[4800..].iter().fold(0.0f32, |p, v| p.max(v.abs()));
        assert!(
            first_half_peak < 0.01,
            "first half should be silent, peak {first_half_peak}"
        );
        assert!(
            second_half_peak > 0.05,
            "second half should be audible, peak {second_half_peak}"
        );
    }

    #[test]
    fn reset_returns_to_default_state() {
        let mut s = FormantSynth::new(48_000);
        s.set_target(vowel_target(), 1);
        let mut buf = [0.0f32; 4800];
        s.process(&mut buf);
        s.reset(Params::DEFAULT);
        let mut buf2 = [0.0f32; 256];
        s.process(&mut buf2);
        for v in &buf2 {
            assert!(v.abs() < 1e-6, "post-reset should be silent, got {v}");
        }
    }

    #[test]
    fn distinct_seeds_decorrelate_noise() {
        // Two synths with all-noise excitation (voicing=0, aspiration=0, A1=1)
        // should produce different output streams when seeded differently.
        let target = ParamUpdate {
            voicing: Some(0.0),
            a1: Some(1.0),
            ..Default::default()
        };
        let mut a = FormantSynth::with_seed(48_000, 0xABCD_1234);
        let mut b = FormantSynth::with_seed(48_000, 0x9876_5432);
        a.set_target(target, 1);
        b.set_target(target, 1);
        let mut buf_a = [0.0f32; 1024];
        let mut buf_b = [0.0f32; 1024];
        a.process(&mut buf_a);
        b.process(&mut buf_b);
        let mut diff_count = 0;
        for i in 0..1024 {
            if (buf_a[i] - buf_b[i]).abs() > 1e-6 {
                diff_count += 1;
            }
        }
        assert!(
            diff_count > 100,
            "expected substantially different output, only {diff_count} diff"
        );
    }
}
