use crate::amp::stages::Stage;
use std::f32::consts::PI;

/// Flush denormals to zero to avoid CPU spikes at silence
#[inline]
fn zap_denormal(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

/// Linkwitz-Riley 4th order crossover filter (cascaded 2nd order Butterworth)
/// This creates a flat summed response at the crossover frequency
#[derive(Clone)]
struct LR4Filter {
    // First biquad state
    x1_1: f32,
    x2_1: f32,
    y1_1: f32,
    y2_1: f32,
    // Second biquad state
    x1_2: f32,
    x2_2: f32,
    y1_2: f32,
    y2_2: f32,
    // Coefficients (same for both biquads)
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    is_highpass: bool,
}

impl LR4Filter {
    fn new(cutoff_hz: f32, sample_rate: f32, is_highpass: bool) -> Self {
        let mut filter = Self {
            x1_1: 0.0,
            x2_1: 0.0,
            y1_1: 0.0,
            y2_1: 0.0,
            x1_2: 0.0,
            x2_2: 0.0,
            y1_2: 0.0,
            y2_2: 0.0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            is_highpass,
        };
        filter.set_cutoff(cutoff_hz, sample_rate);
        filter
    }

    fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate: f32) {
        // Butterworth Q for LR4 cascade
        let q = std::f32::consts::FRAC_1_SQRT_2;
        let omega = 2.0 * PI * cutoff_hz / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);

        let a0 = 1.0 + alpha;

        if self.is_highpass {
            self.b0 = ((1.0 + cos_omega) / 2.0) / a0;
            self.b1 = (-(1.0 + cos_omega)) / a0;
            self.b2 = ((1.0 + cos_omega) / 2.0) / a0;
        } else {
            self.b0 = ((1.0 - cos_omega) / 2.0) / a0;
            self.b1 = (1.0 - cos_omega) / a0;
            self.b2 = ((1.0 - cos_omega) / 2.0) / a0;
        }
        self.a1 = (-2.0 * cos_omega) / a0;
        self.a2 = (1.0 - alpha) / a0;

        // Reset state to avoid clicks when changing cutoff
        self.reset_state();
    }

    fn reset_state(&mut self) {
        self.x1_1 = 0.0;
        self.x2_1 = 0.0;
        self.y1_1 = 0.0;
        self.y2_1 = 0.0;
        self.x1_2 = 0.0;
        self.x2_2 = 0.0;
        self.y1_2 = 0.0;
        self.y2_2 = 0.0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // First biquad
        let y1 = self.b0 * input + self.b1 * self.x1_1 + self.b2 * self.x2_1
            - self.a1 * self.y1_1
            - self.a2 * self.y2_1;
        self.x2_1 = self.x1_1;
        self.x1_1 = input;
        self.y2_1 = self.y1_1;
        self.y1_1 = zap_denormal(y1);

        // Second biquad (cascade)
        let y2 = self.b0 * self.y1_1 + self.b1 * self.x1_2 + self.b2 * self.x2_2
            - self.a1 * self.y1_2
            - self.a2 * self.y2_2;
        self.x2_2 = self.x1_2;
        self.x1_2 = self.y1_1;
        self.y2_2 = self.y1_2;
        self.y1_2 = zap_denormal(y2);

        self.y1_2
    }
}

/// DC blocker to remove any DC offset introduced by saturation
#[derive(Clone)]
struct DcBlocker {
    x_prev: f32,
    y_prev: f32,
    coeff: f32,
}

impl DcBlocker {
    fn new(sample_rate: f32) -> Self {
        // 15 Hz cutoff for DC blocking
        let coeff = (-2.0 * PI * 15.0 / sample_rate).exp();
        Self {
            x_prev: 0.0,
            y_prev: 0.0,
            coeff,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = input - self.x_prev + self.coeff * self.y_prev;
        self.x_prev = input;
        self.y_prev = zap_denormal(output);
        self.y_prev
    }
}

/// Simple envelope follower with configurable attack/release
#[derive(Clone)]
struct EnvelopeFollower {
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl EnvelopeFollower {
    fn new(attack_ms: f32, release_ms: f32, sample_rate: f32) -> Self {
        Self {
            envelope: 0.0,
            attack_coeff: (-1.0 / (attack_ms * 0.001 * sample_rate)).exp(),
            release_coeff: (-1.0 / (release_ms * 0.001 * sample_rate)).exp(),
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();
        if abs_input > self.envelope {
            self.envelope =
                self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_input;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_input;
        }
        self.envelope = zap_denormal(self.envelope);
        self.envelope
    }
}

/// Low band saturator: Exponential curve for tight, focused bass
/// Generates mostly low-order harmonics, minimal aliasing risk
/// Sounds "transformer/power amp" rather than fuzz
#[inline]
fn saturate_low(x: f32, drive: f32) -> f32 {
    let d = 1.0 + drive * 4.0;
    x.signum() * (1.0 - (-x.abs() * d).exp())
}

/// Mid band saturator: Genuinely asymmetric tanh for tube-like character
/// DC bias before tanh generates even harmonics (2nd, 4th, etc.)
/// This is where the "amp voice" and tube warmth lives
#[inline]
fn saturate_mid(x: f32, drive: f32) -> f32 {
    let d = 1.0 + drive * 6.0;
    let bias = 0.15 * drive; // Asymmetry amount scales with drive
    let y = ((x + bias) * d).tanh();
    // Remove the DC offset introduced by the bias
    y - (bias * d).tanh()
}

/// High band saturator: Very mild tanh to control fizz
/// High harmonics alias easily, so we keep saturation gentle
/// Preserves clarity and pick attack without harshness
#[inline]
fn saturate_high(x: f32, drive: f32) -> f32 {
    let d = 1.0 + drive * 2.0;
    (x * d).tanh()
}

pub struct MultibandSaturatorStage {
    // Crossover filters for low/mid split
    low_lp: LR4Filter,
    low_hp: LR4Filter, // HP at low_freq, feeds mid+high
    // Crossover filters for mid/high split (fed from low_hp output)
    mid_lp: LR4Filter,
    high_hp: LR4Filter,

    // High band envelope follower for dynamic fizz control
    high_env: EnvelopeFollower,

    // Single DC blocker at output
    dc_blocker: DcBlocker,

    // Parameters
    low_drive: f32,
    mid_drive: f32,
    high_drive: f32,
    low_level: f32,
    mid_level: f32,
    high_level: f32,
    low_freq: f32,
    high_freq: f32,

    sample_rate: f32,
}

impl MultibandSaturatorStage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        low_drive: f32,
        mid_drive: f32,
        high_drive: f32,
        low_level: f32,
        mid_level: f32,
        high_level: f32,
        low_freq: f32,
        high_freq: f32,
        sample_rate: f32,
    ) -> Self {
        // Clamp frequencies first, including Nyquist guard
        let nyquist = 0.5 * sample_rate;
        let low_freq = low_freq.clamp(50.0, 500.0).min(nyquist * 0.49);
        let high_freq = high_freq.clamp(1000.0, 6000.0).min(nyquist * 0.49);

        Self {
            // Low/mid+high crossover
            low_lp: LR4Filter::new(low_freq, sample_rate, false),
            low_hp: LR4Filter::new(low_freq, sample_rate, true),
            // Mid/high crossover (both fed from low_hp output for proper LR4 summing)
            mid_lp: LR4Filter::new(high_freq, sample_rate, false),
            high_hp: LR4Filter::new(high_freq, sample_rate, true),

            // High band envelope for dynamic fizz control
            // Fast attack (1ms), quick release (30ms) for transient response
            high_env: EnvelopeFollower::new(1.0, 30.0, sample_rate),

            dc_blocker: DcBlocker::new(sample_rate),

            low_drive: low_drive.clamp(0.0, 1.0),
            mid_drive: mid_drive.clamp(0.0, 1.0),
            high_drive: high_drive.clamp(0.0, 1.0),
            low_level: low_level.clamp(0.0, 2.0),
            mid_level: mid_level.clamp(0.0, 2.0),
            high_level: high_level.clamp(0.0, 2.0),
            low_freq,
            high_freq,

            sample_rate,
        }
    }

    fn update_low_crossover(&mut self) {
        self.low_lp.set_cutoff(self.low_freq, self.sample_rate);
        self.low_hp.set_cutoff(self.low_freq, self.sample_rate);
    }

    fn update_high_crossover(&mut self) {
        self.mid_lp.set_cutoff(self.high_freq, self.sample_rate);
        self.high_hp.set_cutoff(self.high_freq, self.sample_rate);
    }
}

impl Stage for MultibandSaturatorStage {
    fn process(&mut self, input: f32) -> f32 {
        // Split into three bands using proper LR4 crossover topology
        // Low band: LP at low_freq
        let low = self.low_lp.process(input);

        // Rest (mid+high): HP at low_freq
        let rest = self.low_hp.process(input);

        // Mid band: rest -> LP at high_freq
        let mid = self.mid_lp.process(rest);

        // High band: rest -> HP at high_freq (proper LR4 summing)
        let high = self.high_hp.process(rest);

        // Track envelope for high band fizz control
        let high_env = self.high_env.process(high);

        // Apply band-specific saturation
        // No automatic makeup gain - users control loudness via level parameters
        let low_sat = saturate_low(low, self.low_drive);
        let mid_sat = saturate_mid(mid, self.mid_drive);

        // High band: use envelope to dynamically tame fizz
        // When highs get loud, reduce input to saturator to prevent harsh aliasing artifacts
        let fizz_control = 1.0 / (1.0 + high_env * 2.0);
        let high_sat = saturate_high(high * fizz_control, self.high_drive);

        // Mix bands with level controls
        let mixed =
            low_sat * self.low_level + mid_sat * self.mid_level + high_sat * self.high_level;

        // Single DC blocker at output (saturation is symmetric, minimal DC expected)
        self.dc_blocker.process(mixed)
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "low_drive" => {
                if (0.0..=1.0).contains(&value) {
                    self.low_drive = value;
                    Ok(())
                } else {
                    Err("Low drive must be 0.0-1.0")
                }
            }
            "mid_drive" => {
                if (0.0..=1.0).contains(&value) {
                    self.mid_drive = value;
                    Ok(())
                } else {
                    Err("Mid drive must be 0.0-1.0")
                }
            }
            "high_drive" => {
                if (0.0..=1.0).contains(&value) {
                    self.high_drive = value;
                    Ok(())
                } else {
                    Err("High drive must be 0.0-1.0")
                }
            }
            "low_level" => {
                if (0.0..=2.0).contains(&value) {
                    self.low_level = value;
                    Ok(())
                } else {
                    Err("Low level must be 0.0-2.0")
                }
            }
            "mid_level" => {
                if (0.0..=2.0).contains(&value) {
                    self.mid_level = value;
                    Ok(())
                } else {
                    Err("Mid level must be 0.0-2.0")
                }
            }
            "high_level" => {
                if (0.0..=2.0).contains(&value) {
                    self.high_level = value;
                    Ok(())
                } else {
                    Err("High level must be 0.0-2.0")
                }
            }
            "low_freq" => {
                let nyquist = 0.5 * self.sample_rate;
                let max_freq = 500.0_f32.min(nyquist * 0.49);
                if (50.0..=max_freq).contains(&value) {
                    self.low_freq = value;
                    self.update_low_crossover();
                    Ok(())
                } else {
                    Err("Low freq must be 50-500 Hz (and below Nyquist)")
                }
            }
            "high_freq" => {
                let nyquist = 0.5 * self.sample_rate;
                let max_freq = 6000.0_f32.min(nyquist * 0.49);
                if (1000.0..=max_freq).contains(&value) {
                    self.high_freq = value;
                    self.update_high_crossover();
                    Ok(())
                } else {
                    Err("High freq must be 1000-6000 Hz (and below Nyquist)")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "low_drive" => Ok(self.low_drive),
            "mid_drive" => Ok(self.mid_drive),
            "high_drive" => Ok(self.high_drive),
            "low_level" => Ok(self.low_level),
            "mid_level" => Ok(self.mid_level),
            "high_level" => Ok(self.high_level),
            "low_freq" => Ok(self.low_freq),
            "high_freq" => Ok(self.high_freq),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiband_saturator_default() {
        let mut stage = MultibandSaturatorStage::new(
            0.5, 0.5, 0.5, // drives
            1.0, 1.0, 1.0, // levels
            200.0, 2000.0,  // crossover frequencies
            48000.0, // sample rate
        );

        // Process some samples to verify no panic
        for _ in 0..1000 {
            let _ = stage.process(0.5);
        }
    }

    #[test]
    fn test_parameter_bounds() {
        let mut stage =
            MultibandSaturatorStage::new(0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 200.0, 2000.0, 48000.0);

        // Valid parameters
        assert!(stage.set_parameter("low_drive", 0.0).is_ok());
        assert!(stage.set_parameter("low_drive", 1.0).is_ok());
        assert!(stage.set_parameter("low_level", 0.0).is_ok());
        assert!(stage.set_parameter("low_level", 2.0).is_ok());
        assert!(stage.set_parameter("low_freq", 50.0).is_ok());
        assert!(stage.set_parameter("low_freq", 500.0).is_ok());
        assert!(stage.set_parameter("high_freq", 1000.0).is_ok());
        assert!(stage.set_parameter("high_freq", 6000.0).is_ok());

        // Invalid parameters
        assert!(stage.set_parameter("low_drive", -0.1).is_err());
        assert!(stage.set_parameter("low_drive", 1.1).is_err());
        assert!(stage.set_parameter("low_level", -0.1).is_err());
        assert!(stage.set_parameter("low_level", 2.1).is_err());
        assert!(stage.set_parameter("low_freq", 49.0).is_err());
        assert!(stage.set_parameter("high_freq", 6001.0).is_err());
        assert!(stage.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn test_get_parameters() {
        let stage =
            MultibandSaturatorStage::new(0.3, 0.5, 0.7, 0.8, 1.0, 1.2, 150.0, 3000.0, 48000.0);

        assert!((stage.get_parameter("low_drive").unwrap() - 0.3).abs() < 0.001);
        assert!((stage.get_parameter("mid_drive").unwrap() - 0.5).abs() < 0.001);
        assert!((stage.get_parameter("high_drive").unwrap() - 0.7).abs() < 0.001);
        assert!((stage.get_parameter("low_level").unwrap() - 0.8).abs() < 0.001);
        assert!((stage.get_parameter("mid_level").unwrap() - 1.0).abs() < 0.001);
        assert!((stage.get_parameter("high_level").unwrap() - 1.2).abs() < 0.001);
        assert!((stage.get_parameter("low_freq").unwrap() - 150.0).abs() < 0.001);
        assert!((stage.get_parameter("high_freq").unwrap() - 3000.0).abs() < 0.001);
        assert!(stage.get_parameter("unknown").is_err());
    }

    #[test]
    fn test_saturation_functions() {
        // Test low band saturator (exponential curve)
        let low_clean = saturate_low(0.5, 0.0);
        assert!(low_clean > 0.0 && low_clean <= 1.0);
        let low_driven = saturate_low(0.5, 1.0);
        assert!(low_driven > low_clean); // More drive = more output
        assert!(low_driven <= 1.0); // Bounded

        // Test mid band saturator (genuinely asymmetric)
        let mid_clean = saturate_mid(0.5, 0.0);
        assert!((mid_clean - 0.5_f32.tanh()).abs() < 0.01); // At drive=0, bias=0
        let mid_driven = saturate_mid(0.5, 1.0);
        assert!(mid_driven.abs() <= 1.5); // Bounded (with some headroom for asymmetry)

        // Verify asymmetry: positive and negative inputs should produce different magnitudes
        let mid_pos = saturate_mid(0.5, 1.0);
        let mid_neg = saturate_mid(-0.5, 1.0);
        assert!((mid_pos.abs() - mid_neg.abs()).abs() > 0.01); // Not symmetric

        // Test high band saturator (gentle tanh)
        let high_clean = saturate_high(0.5, 0.0);
        assert!((high_clean - 0.5_f32.tanh()).abs() < 0.001);
        let high_driven = saturate_high(0.5, 1.0);
        assert!(high_driven.abs() <= 1.0);

        // Test negative values preserve sign
        assert!(saturate_low(-0.5, 0.5) < 0.0);
        assert!(saturate_mid(-0.5, 0.5) < 0.0);
        assert!(saturate_high(-0.5, 0.5) < 0.0);
    }

    #[test]
    fn test_dc_blocking() {
        let mut stage =
            MultibandSaturatorStage::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 200.0, 2000.0, 48000.0);

        // Process DC offset signal
        let mut last_output = 0.0;
        for _ in 0..10000 {
            last_output = stage.process(1.0);
        }

        // DC should be mostly blocked
        assert!(last_output.abs() < 0.1);
    }

    #[test]
    fn test_frequency_clamping() {
        // Test that out-of-range frequencies are clamped
        let stage = MultibandSaturatorStage::new(
            0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 10.0,    // too low
            10000.0, // too high
            48000.0,
        );

        assert!((stage.get_parameter("low_freq").unwrap() - 50.0).abs() < 0.001);
        assert!((stage.get_parameter("high_freq").unwrap() - 6000.0).abs() < 0.001);
    }

    #[test]
    fn test_nyquist_guard() {
        // At 8000 Hz sample rate, Nyquist is 4000 Hz
        // high_freq should be clamped to 4000 * 0.49 = 1960 Hz
        let stage = MultibandSaturatorStage::new(
            0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 200.0, 3000.0, // would be above Nyquist
            8000.0,
        );

        let high_freq = stage.get_parameter("high_freq").unwrap();
        assert!(high_freq < 4000.0 * 0.5); // Must be below Nyquist
    }

    #[test]
    fn test_denormal_protection() {
        let mut stage =
            MultibandSaturatorStage::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 200.0, 2000.0, 48000.0);

        // Process silence for a while - should not produce denormals
        for _ in 0..100000 {
            let out = stage.process(0.0);
            // Output must be finite (not NaN/inf)
            assert!(out.is_finite());
            // Output must not be subnormal (denormal)
            assert!(out == 0.0 || out.abs() >= f32::MIN_POSITIVE);
        }
    }
}
