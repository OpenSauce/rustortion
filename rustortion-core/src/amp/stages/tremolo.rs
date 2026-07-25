use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::amp::stages::Stage;
use crate::amp::stages::common::calculate_coefficient;

const MIN_RATE_HZ: f32 = 0.1;
const MAX_RATE_HZ: f32 = 20.0;

/// Pure-sine floor: `tanh` drive at exactly `shape = 0.0`. `tanh` is ~linear
/// near zero, so `tanh(raw * d) / tanh(d)` collapses back to `raw` — a faithful
/// sine. Kept above zero to avoid a `0 / 0`.
const MIN_DRIVE: f32 = 1e-3;

/// Start of the audible morph sweep, for `shape` just above 0. Drive sweeps
/// *geometrically* from here to `MAX_DRIVE`, because the sine→square morph
/// saturates with drive: a linear sweep finishes ~80% of the morph in the
/// bottom third of the knob, leaving the upper half inert. Equal `shape` steps
/// → equal drive *ratios* spreads the morph evenly across the control.
const SHAPE_MIN_DRIVE: f32 = 0.5;

/// `tanh` drive at `shape = 1.0`. ~16 gives a hard, crisp square for the
/// "killswitch" chop without the aliasing of a literal `sign()`.
const MAX_DRIVE: f32 = 16.0;

/// One-pole smoothing time for the depth parameter — fast enough to feel
/// instant, slow enough to suppress zipper noise when the slider is dragged.
/// The LFO output itself is never smoothed, so square edges stay crisp.
const DEPTH_SMOOTH_MS: f32 = 30.0;

/// Tremolo — amplitude modulation by a low-frequency oscillator.
///
/// A phase accumulator drives a sine LFO. The `shape` parameter morphs that
/// sine toward a hard square via `tanh` waveshaping, so a single stage spans
/// vintage tremolo (`shape = 0`) through a square-wave "killswitch" stutter
/// (`shape = 1`, `depth = 1`). The modulator is mapped to a gain in
/// `[1 - depth, 1]`, so at full depth the signal dips all the way to silence at
/// each trough.
pub struct TremoloStage {
    rate_hz: f32,
    depth: f32,
    shape: f32,
    sample_rate: f32,
    phase: f32,
    depth_smoothed: f32,
    depth_coeff: f32,
    /// Cached shape-morph coefficients. `drive` and `drive_norm = 1/tanh(drive)`
    /// depend only on `shape`, so they're computed in `new()` and on
    /// `set_parameter("shape", _)` — never per sample (saves one `tanh()`/sample).
    drive: f32,
    drive_norm: f32,
}

impl TremoloStage {
    pub fn new(rate_hz: f32, depth: f32, shape: f32, sample_rate: f32) -> Self {
        let rate_hz = rate_hz.clamp(MIN_RATE_HZ, MAX_RATE_HZ);
        let depth = depth.clamp(0.0, 1.0);
        let shape = shape.clamp(0.0, 1.0);

        let mut stage = Self {
            rate_hz,
            depth,
            shape,
            sample_rate,
            phase: 0.0,
            depth_smoothed: depth,
            depth_coeff: calculate_coefficient(DEPTH_SMOOTH_MS, sample_rate),
            drive: 0.0,
            drive_norm: 1.0,
        };
        stage.update_shape_coeffs();
        stage
    }

    /// Recompute the cached shape-morph coefficients. `drive` maps `shape` onto
    /// the `tanh` waveshaper's slope; `drive_norm = 1 / tanh(drive)` renormalises
    /// the shaped output back to ±1. Called only when `shape` changes.
    fn update_shape_coeffs(&mut self) {
        // Geometric sweep so the morph spreads evenly across the knob (see
        // SHAPE_MIN_DRIVE). `shape` exactly 0 stays a true sine; above that,
        // drive ramps from SHAPE_MIN_DRIVE up to MAX_DRIVE.
        self.drive = if self.shape <= 0.0 {
            MIN_DRIVE
        } else {
            SHAPE_MIN_DRIVE * (MAX_DRIVE / SHAPE_MIN_DRIVE).powf(self.shape)
        };
        self.drive_norm = 1.0 / self.drive.tanh();
    }

    /// Current LFO gain in `[1 - depth, 1]`, advancing the phase by one sample.
    fn next_gain(&mut self) -> f32 {
        // Smooth depth to avoid zipper noise; the LFO output stays unsmoothed.
        self.depth_smoothed = self
            .depth_coeff
            .mul_add(self.depth_smoothed, (1.0 - self.depth_coeff) * self.depth);

        let raw = (TAU * self.phase).sin();

        // Morph sine -> square via the cached coefficients (see
        // `update_shape_coeffs`). At `MIN_DRIVE` this reproduces `raw`; at
        // `MAX_DRIVE` it clamps to ~±1 everywhere but the zero crossings.
        let sharp = (raw * self.drive).tanh() * self.drive_norm;

        // Map [-1, 1] -> [0, 1], then to a gain in [1 - depth, 1]:
        //   gain = 1 - depth * (1 - m) = depth * (m - 1) + 1
        let m = 0.5f32.mul_add(sharp, 0.5);
        let gain = self.depth_smoothed.mul_add(m - 1.0, 1.0);

        // Advance phase, wrapping to [0, 1).
        self.phase += self.rate_hz / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        gain
    }
}

impl Stage for TremoloStage {
    fn process(&mut self, input: f32) -> f32 {
        input * self.next_gain()
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "rate" => {
                if (MIN_RATE_HZ..=MAX_RATE_HZ).contains(&value) {
                    self.rate_hz = value;
                    Ok(())
                } else {
                    Err("Rate must be between 0.1 Hz and 20 Hz")
                }
            }
            "depth" => {
                if (0.0..=1.0).contains(&value) {
                    self.depth = value;
                    Ok(())
                } else {
                    Err("Depth must be between 0.0 and 1.0")
                }
            }
            "shape" => {
                if (0.0..=1.0).contains(&value) {
                    self.shape = value;
                    self.update_shape_coeffs();
                    Ok(())
                } else {
                    Err("Shape must be between 0.0 and 1.0")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "rate" => Ok(self.rate_hz),
            "depth" => Ok(self.depth),
            "shape" => Ok(self.shape),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;
    const TOL: f32 = 1e-3;

    #[test]
    fn depth_zero_is_unity_passthrough() {
        // depth 0 => gain is always 1.0, regardless of rate/shape.
        let mut trem = TremoloStage::new(5.0, 0.0, 0.5, SAMPLE_RATE);
        for i in 0..2000 {
            let input = (i as f32 * 0.01).sin();
            let out = trem.process(input);
            assert!(
                (out - input).abs() < TOL,
                "depth 0 should pass dry at sample {i}: in {input}, out {out}"
            );
        }
    }

    #[test]
    fn gain_stays_in_unit_range() {
        // For any depth/shape, the applied gain (and thus a unit DC input) must
        // never leave [0, 1] — no boost, no phase inversion.
        for &(depth, shape) in &[(0.3, 0.0), (0.7, 0.5), (1.0, 1.0), (0.5, 1.0)] {
            let mut trem = TremoloStage::new(7.0, depth, shape, SAMPLE_RATE);
            for _ in 0..(SAMPLE_RATE as usize) {
                let out = trem.process(1.0);
                assert!(
                    (-TOL..=1.0 + TOL).contains(&out),
                    "gain out of range at depth {depth}, shape {shape}: {out}"
                );
            }
        }
    }

    #[test]
    fn full_chop_reaches_silence_and_unity() {
        // depth 1 + shape 1 (square) = killswitch: gain alternates ~0 and ~1.
        let mut trem = TremoloStage::new(5.0, 1.0, 1.0, SAMPLE_RATE);
        let mut min_gain = f32::INFINITY;
        let mut max_gain = f32::NEG_INFINITY;
        // Two full periods at 5 Hz.
        for _ in 0..((SAMPLE_RATE as usize) * 2 / 5) {
            let g = trem.process(1.0);
            min_gain = min_gain.min(g);
            max_gain = max_gain.max(g);
        }
        assert!(min_gain < 0.02, "trough should mute, got {min_gain}");
        assert!(max_gain > 0.98, "peak should pass unity, got {max_gain}");
    }

    #[test]
    fn shape_zero_tracks_sine() {
        // At shape 0 + depth 1, gain == 0.5 * (sin(2*pi*phase) + 1).
        let rate = 10.0;
        let mut trem = TremoloStage::new(rate, 1.0, 0.0, SAMPLE_RATE);
        for i in 0..4410 {
            let g = trem.process(1.0);
            let phase = i as f32 * rate / SAMPLE_RATE;
            let expected = 0.5f32.mul_add((TAU * phase).sin(), 0.5);
            assert!(
                (g - expected).abs() < TOL,
                "sine mismatch at {i}: got {g}, expected {expected}"
            );
        }
    }

    #[test]
    fn lfo_is_periodic() {
        // 10 Hz at 44.1 kHz => exactly 4410 samples per cycle. The gain at
        // sample i must match the gain at sample i + period.
        let rate = 10.0;
        let period = 4410usize;
        let mut trem = TremoloStage::new(rate, 0.8, 0.4, SAMPLE_RATE);
        let mut gains = Vec::with_capacity(period * 2 + 8);
        for _ in 0..(period * 2 + 8) {
            gains.push(trem.process(1.0));
        }
        for i in (0..period).step_by(137) {
            assert!(
                (gains[i] - gains[i + period]).abs() < TOL,
                "not periodic at {i}: {} vs {}",
                gains[i],
                gains[i + period]
            );
        }
    }

    #[test]
    fn upper_shape_knob_still_morphs() {
        // Regression guard: the upper half of the Shape knob must keep morphing
        // toward square. A linear drive sweep saturated by ~shape 0.5, making
        // 0.5→1.0 nearly identical (MAE ~0.019); the geometric sweep keeps them
        // distinct (MAE ~0.066). Compare one full LFO cycle at depth 1.
        fn cycle_gains(shape: f32) -> Vec<f32> {
            let mut s = TremoloStage::new(5.0, 1.0, shape, SAMPLE_RATE);
            let n = (SAMPLE_RATE / 5.0) as usize;
            (0..n).map(|_| s.process(1.0)).collect()
        }
        let mid = cycle_gains(0.5);
        let top = cycle_gains(1.0);
        let mae: f32 = mid
            .iter()
            .zip(&top)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / mid.len() as f32;
        assert!(mae > 0.03, "upper-half Shape morph too flat (mae={mae})");
    }

    #[test]
    fn set_shape_matches_constructed() {
        // Changing shape via set_parameter must refresh the cached drive/norm,
        // so output matches a stage built with that shape from the start.
        let mut a = TremoloStage::new(7.0, 0.8, 0.0, SAMPLE_RATE);
        a.set_parameter("shape", 1.0).unwrap();
        let mut b = TremoloStage::new(7.0, 0.8, 1.0, SAMPLE_RATE);
        for i in 0..2000 {
            let (oa, ob) = (a.process(1.0), b.process(1.0));
            assert!(
                (oa - ob).abs() < TOL,
                "stale shape cache at {i}: {oa} vs {ob}"
            );
        }
    }

    #[test]
    fn parameter_validation() {
        let mut trem = TremoloStage::new(5.0, 0.5, 0.0, SAMPLE_RATE);

        assert!(trem.set_parameter("rate", 0.05).is_err());
        assert!(trem.set_parameter("rate", 25.0).is_err());
        assert!(trem.set_parameter("rate", 12.0).is_ok());

        assert!(trem.set_parameter("depth", -0.1).is_err());
        assert!(trem.set_parameter("depth", 1.1).is_err());
        assert!(trem.set_parameter("depth", 0.75).is_ok());

        assert!(trem.set_parameter("shape", -0.1).is_err());
        assert!(trem.set_parameter("shape", 1.1).is_err());
        assert!(trem.set_parameter("shape", 1.0).is_ok());

        assert!(trem.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn constructor_clamps_out_of_range() {
        let trem = TremoloStage::new(100.0, 2.0, 2.0, SAMPLE_RATE);
        assert!((trem.get_parameter("rate").unwrap() - MAX_RATE_HZ).abs() < TOL);
        assert!((trem.get_parameter("depth").unwrap() - 1.0).abs() < TOL);
        assert!((trem.get_parameter("shape").unwrap() - 1.0).abs() < TOL);

        let trem = TremoloStage::new(0.0, -1.0, -1.0, SAMPLE_RATE);
        assert!((trem.get_parameter("rate").unwrap() - MIN_RATE_HZ).abs() < TOL);
        assert!(trem.get_parameter("depth").unwrap().abs() < TOL);
        assert!(trem.get_parameter("shape").unwrap().abs() < TOL);
    }

    #[test]
    fn get_parameters() {
        let trem = TremoloStage::new(8.0, 0.6, 0.3, SAMPLE_RATE);
        assert!((trem.get_parameter("rate").unwrap() - 8.0).abs() < TOL);
        assert!((trem.get_parameter("depth").unwrap() - 0.6).abs() < TOL);
        assert!((trem.get_parameter("shape").unwrap() - 0.3).abs() < TOL);
        assert!(trem.get_parameter("unknown").is_err());
    }

    #[test]
    fn default_config() {
        let cfg = TremoloConfig::default();
        assert!((cfg.rate_hz - 5.0).abs() < TOL);
        assert!((cfg.depth - 0.5).abs() < TOL);
        assert!((cfg.shape - 0.0).abs() < TOL);
        assert!(!cfg.bypassed);
    }
}

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TremoloConfig {
    pub rate_hz: f32,
    pub depth: f32,
    pub shape: f32,
    #[serde(default)]
    pub bypassed: bool,
}

impl Default for TremoloConfig {
    fn default() -> Self {
        Self {
            rate_hz: 5.0,
            depth: 0.5,
            shape: 0.0,
            bypassed: false,
        }
    }
}

impl TremoloConfig {
    pub fn to_stage(&self, sample_rate: f32) -> TremoloStage {
        TremoloStage::new(self.rate_hz, self.depth, self.shape, sample_rate)
    }
}
