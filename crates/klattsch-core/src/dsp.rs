//! Bandpass biquad, Rosenberg glottal pulse, 32-bit xorshift LFSR, soft-clip.

use core::f64::consts::PI;

/// Constant-skirt-gain bandpass biquad (RBJ Audio EQ Cookbook). Coefficients
/// are recomputed only when frequency or bandwidth changes.
#[derive(Clone, Copy, Debug)]
pub struct BandpassBiquad {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    last_f: f32,
    last_bw: f32,
}

impl BandpassBiquad {
    pub const fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            last_f: -1.0,
            last_bw: -1.0,
        }
    }

    /// Update coefficients if (f, bw) differ from the previous call.
    /// Frequency is clamped to `[40 Hz, sr * 0.45]`, bandwidth to `>= 20 Hz`.
    #[inline]
    pub fn set_freq(&mut self, f: f32, bw: f32, sr: f32) {
        if f == self.last_f && bw == self.last_bw {
            return;
        }
        self.last_f = f;
        self.last_bw = bw;
        let f = (f as f64).max(40.0).min(sr as f64 * 0.45);
        let bw = (bw as f64).max(20.0);
        let w0 = 2.0 * PI * f / sr as f64;
        let cosw0 = w0.cos();
        let sinw0 = w0.sin();
        let q = f / bw;
        let alpha = sinw0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.b0 = alpha / a0;
        self.b1 = 0.0;
        self.b2 = -alpha / a0;
        self.a1 = -2.0 * cosw0 / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y as f32
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

impl Default for BandpassBiquad {
    fn default() -> Self {
        Self::new()
    }
}

/// Derivative of the Rosenberg glottal pulse. Phase is normalized to `[0, 1)`.
/// `effort` (0..1) shapes the pulse: 0 is lax/breathy, 1 is tense.
#[inline]
pub fn glottal_pulse(phase: f64, effort: f32) -> f32 {
    let e = effort.clamp(0.0, 1.0) as f64;
    let tp = 0.5 - e * 0.2; // 0.5 (lax) -> 0.3 (tense)
    let tn = 0.25 - e * 0.17; // 0.25 (lax) -> 0.08 (tense)
    const NORM: f64 = 0.1;
    let result = if phase < tp {
        NORM * 0.5 * (PI / tp) * (PI * phase / tp).sin()
    } else if phase < tp + tn {
        -NORM * (PI / (2.0 * tn)) * (PI * (phase - tp) / (2.0 * tn)).sin()
    } else {
        0.0
    };
    result as f32
}

/// 32-bit xorshift LFSR.
#[derive(Clone, Copy, Debug)]
pub struct Xorshift32 {
    state: i32,
}

impl Xorshift32 {
    pub const DEFAULT_SEED: u32 = 0xACE1_ACE1;

    pub const fn new() -> Self {
        Self {
            state: Self::DEFAULT_SEED as i32,
        }
    }

    pub const fn with_seed(seed: u32) -> Self {
        Self { state: seed as i32 }
    }

    /// Advance the LFSR and return a sample in `[-1, 1)`.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x.wrapping_shl(13);
        x = ((x as u32) >> 17) as i32 ^ x;
        x ^= x.wrapping_shl(5);
        self.state = x;
        (x as f32) / 2_147_483_648.0
    }

    pub fn reset(&mut self, seed: u32) {
        self.state = seed as i32;
    }
}

impl Default for Xorshift32 {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const T: f32 = 0.85;
    let a = x.abs();
    if a <= T {
        return x;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let excess = a - T;
    sign * (T + (1.0 - T) * excess / (excess + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biquad_silent_with_zero_input() {
        let mut bp = BandpassBiquad::new();
        bp.set_freq(1000.0, 100.0, 48_000.0);
        for _ in 0..1000 {
            let y = bp.process(0.0);
            assert_eq!(y, 0.0);
        }
    }

    #[test]
    fn set_freq_is_idempotent_for_identical_args() {
        let mut bp = BandpassBiquad::new();
        bp.set_freq(500.0, 100.0, 48_000.0);
        let first = (bp.b0, bp.b1, bp.b2, bp.a1, bp.a2);
        bp.set_freq(500.0, 100.0, 48_000.0); // identical args -> early return
        assert_eq!((bp.b0, bp.b1, bp.b2, bp.a1, bp.a2), first);
    }

    #[test]
    fn biquad_resonates_at_center() {
        // Drive a 500 Hz biquad with a 500 Hz sinusoid; output amplitude should
        // exceed input amplitude (resonance peak) once it has settled.
        let sr_f = 48_000.0_f32;
        let f = 500.0_f32;
        let mut bp = BandpassBiquad::new();
        bp.set_freq(f, 50.0, sr_f);
        let mut peak = 0.0f32;
        for n in 0..(sr_f as usize) {
            let x = (2.0 * core::f32::consts::PI * f * (n as f32) / sr_f).sin();
            let y = bp.process(x);
            // Skip transient
            if n > sr_f as usize / 4 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak > 1.0, "expected resonance, got peak {peak}");
    }

    #[test]
    fn glottal_pulse_zero_after_close() {
        // For phase past Tp + Tn, output is exactly zero (closed phase).
        // With effort=0.5, Tp=0.4, Tn=0.165 → closed phase starts at 0.565.
        assert_eq!(glottal_pulse(0.7, 0.5), 0.0);
        assert_eq!(glottal_pulse(0.99, 0.5), 0.0);
    }

    #[test]
    fn glottal_pulse_clamps_effort() {
        assert_eq!(glottal_pulse(0.1, -1.0), glottal_pulse(0.1, 0.0));
        assert_eq!(glottal_pulse(0.1, 2.0), glottal_pulse(0.1, 1.0));
    }

    #[test]
    fn xorshift_matches_js_first_samples() {
        let mut rng = Xorshift32::new();

        let s1 = rng.next_sample();
        assert_eq!(rng.state as u32, 0xb6c5_cbbf);
        assert_eq!(s1.to_bits(), 0xbf12_7469);

        let s2 = rng.next_sample();
        assert_eq!(rng.state as u32, 0xf9f7_a0a6);
        assert_eq!(s2.to_bits(), 0xbd41_0beb);

        let s3 = rng.next_sample();
        assert_eq!(rng.state as u32, 0xb18f_acb7);
        assert_eq!(s3.to_bits(), 0xbf1c_e0a7);
    }

    #[test]
    fn soft_clip_passes_small_signals_unchanged() {
        for x in [-0.5, -0.85, 0.0, 0.5, 0.85] {
            assert_eq!(soft_clip(x), x);
        }
    }

    #[test]
    fn soft_clip_bounded_above_threshold() {
        for x in [0.9, 1.0, 5.0, 100.0] {
            let y = soft_clip(x);
            assert!(y < 1.0, "soft_clip({x}) = {y} should be < 1");
            assert!(y > 0.85, "soft_clip({x}) = {y} should be > T");
        }
        for x in [-0.9, -1.0, -5.0, -100.0] {
            let y = soft_clip(x);
            assert!(y > -1.0);
            assert!(y < -0.85);
        }
    }
}
