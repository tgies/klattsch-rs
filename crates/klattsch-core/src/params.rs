//! Synthesis parameters and sparse update type.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Full set of synthesis parameters. All fields are continuously interpolated
/// per sample during transitions.
///
/// Units:
/// - `f0`, `f1..f3`, `bw1..bw3`, `vibrato_rate`, `tremolo_rate`: Hz
/// - `voicing`, `aspiration`, `tremolo_depth`, `effort`: dimensionless 0..1
/// - `tilt`: dimensionless -0.95..0.95 (positive = brighter)
/// - `vibrato_depth`: Hz peak deviation around `f0`
/// - `a1..a3`, `gain`: linear amplitude
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Params {
    pub f0: f32,
    pub voicing: f32,

    pub f1: f32,
    pub bw1: f32,
    pub a1: f32,

    pub f2: f32,
    pub bw2: f32,
    pub a2: f32,

    pub f3: f32,
    pub bw3: f32,
    pub a3: f32,

    pub gain: f32,

    pub vibrato_depth: f32,
    pub vibrato_rate: f32,
    pub tremolo_depth: f32,
    pub tremolo_rate: f32,

    pub aspiration: f32,
    pub tilt: f32,
    pub effort: f32,
}

impl Params {
    pub const DEFAULT: Self = Self {
        f0: 120.0,
        voicing: 0.0,
        f1: 500.0,
        bw1: 80.0,
        a1: 0.0,
        f2: 1500.0,
        bw2: 120.0,
        a2: 0.0,
        f3: 2500.0,
        bw3: 160.0,
        a3: 0.0,
        gain: 3.5,
        vibrato_depth: 0.0,
        vibrato_rate: 5.0,
        tremolo_depth: 0.0,
        tremolo_rate: 5.0,
        aspiration: 0.0,
        tilt: 0.0,
        effort: 0.5,
    };
}

impl Default for Params {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Sparse parameter update. Only `Some(_)` fields override; `None` fields
/// leave the corresponding target unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ParamUpdate {
    pub f0: Option<f32>,
    pub voicing: Option<f32>,
    pub f1: Option<f32>,
    pub bw1: Option<f32>,
    pub a1: Option<f32>,
    pub f2: Option<f32>,
    pub bw2: Option<f32>,
    pub a2: Option<f32>,
    pub f3: Option<f32>,
    pub bw3: Option<f32>,
    pub a3: Option<f32>,
    pub gain: Option<f32>,
    pub vibrato_depth: Option<f32>,
    pub vibrato_rate: Option<f32>,
    pub tremolo_depth: Option<f32>,
    pub tremolo_rate: Option<f32>,
    pub aspiration: Option<f32>,
    pub tilt: Option<f32>,
    pub effort: Option<f32>,
}

impl ParamUpdate {
    /// Apply this sparse update to a [`Params`], leaving `None` fields unchanged.
    pub fn apply_to(&self, p: &mut Params) {
        macro_rules! set {
            ($f:ident) => {
                if let Some(v) = self.$f {
                    p.$f = v;
                }
            };
        }
        set!(f0);
        set!(voicing);
        set!(f1);
        set!(bw1);
        set!(a1);
        set!(f2);
        set!(bw2);
        set!(a2);
        set!(f3);
        set!(bw3);
        set!(a3);
        set!(gain);
        set!(vibrato_depth);
        set!(vibrato_rate);
        set!(tremolo_depth);
        set!(tremolo_rate);
        set!(aspiration);
        set!(tilt);
        set!(effort);
    }

    /// Build the per-sample increment that ramps `current` toward `target` over
    /// `n_samples`. For each `Some(_)` field in `target`, computes
    /// `(target - current) / n_samples`. `None` fields stay zero.
    pub fn ramp_increments(target: &ParamUpdate, current: &Params, n_samples: u32) -> ParamUpdate {
        let n = n_samples.max(1) as f32;
        macro_rules! delta {
            ($f:ident) => {
                target.$f.map(|t| (t - current.$f) / n)
            };
        }
        ParamUpdate {
            f0: delta!(f0),
            voicing: delta!(voicing),
            f1: delta!(f1),
            bw1: delta!(bw1),
            a1: delta!(a1),
            f2: delta!(f2),
            bw2: delta!(bw2),
            a2: delta!(a2),
            f3: delta!(f3),
            bw3: delta!(bw3),
            a3: delta!(a3),
            gain: delta!(gain),
            vibrato_depth: delta!(vibrato_depth),
            vibrato_rate: delta!(vibrato_rate),
            tremolo_depth: delta!(tremolo_depth),
            tremolo_rate: delta!(tremolo_rate),
            aspiration: delta!(aspiration),
            tilt: delta!(tilt),
            effort: delta!(effort),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_js_constants() {
        assert_eq!(
            Params::DEFAULT,
            Params {
                f0: 120.0,
                voicing: 0.0,
                f1: 500.0,
                bw1: 80.0,
                a1: 0.0,
                f2: 1500.0,
                bw2: 120.0,
                a2: 0.0,
                f3: 2500.0,
                bw3: 160.0,
                a3: 0.0,
                gain: 3.5,
                vibrato_depth: 0.0,
                vibrato_rate: 5.0,
                tremolo_depth: 0.0,
                tremolo_rate: 5.0,
                aspiration: 0.0,
                tilt: 0.0,
                effort: 0.5,
            }
        );
    }

    #[test]
    fn apply_overlays_only_some_fields() {
        let mut p = Params::DEFAULT;
        let upd = ParamUpdate {
            f0: Some(220.0),
            a1: Some(1.0),
            ..Default::default()
        };
        upd.apply_to(&mut p);
        assert_eq!(p.f0, 220.0);
        assert_eq!(p.a1, 1.0);
        assert_eq!(p.f1, 500.0); // untouched
        assert_eq!(p.gain, 3.5); // untouched
    }

    #[test]
    fn ramp_increments_divides_by_n() {
        let cur = Params::DEFAULT;
        let tgt = ParamUpdate {
            f0: Some(220.0),
            ..Default::default()
        };
        let inc = ParamUpdate::ramp_increments(&tgt, &cur, 100);
        assert!((inc.f0.unwrap() - (220.0 - 120.0) / 100.0).abs() < 1e-6);
        assert_eq!(inc.a1, None);
    }
}
