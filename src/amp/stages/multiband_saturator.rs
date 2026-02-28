use crate::amp::stages::Stage;
use crate::amp::stages::common::{DcBlocker, EnvelopeFollower};
use std::f32::consts::PI;

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
        self.y1_1 = y1;

        // Second biquad (cascade)
        let y2 = self.b0 * y1 + self.b1 * self.x1_2 + self.b2 * self.x2_2
            - self.a1 * self.y1_2
            - self.a2 * self.y2_2;
        self.x2_2 = self.x1_2;
        self.x1_2 = y1;
        self.y2_2 = self.y1_2;
        self.y1_2 = y2;

        y2
    }
}

/// Soft saturation function with drive control
#[inline]
fn saturate(input: f32, drive: f32) -> f32 {
    // Drive scales from 1.0 (clean) to ~10 (heavy saturation)
    let drive_scaled = 1.0 + drive * 9.0;
    let x = input * drive_scaled;
    // Soft clipping bounded to (-1, 1)
    x / (1.0 + x.abs())
}

pub struct MultibandSaturatorStage {
    // Crossover filters for low/mid split
    low_lp: LR4Filter,
    mid_hp_low: LR4Filter,
    // Crossover filters for mid/high split
    mid_lp_high: LR4Filter,
    high_hp: LR4Filter,
    // Allpass phase compensation for low band (LP + HP at high_freq = allpass)
    low_allpass_lp: LR4Filter,
    low_allpass_hp: LR4Filter,

    // Per-band envelope followers
    low_env: EnvelopeFollower,
    mid_env: EnvelopeFollower,
    high_env: EnvelopeFollower,

    // Per-band DC blockers
    low_dc: DcBlocker,
    mid_dc: DcBlocker,
    high_dc: DcBlocker,

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
        Self {
            // Low/mid crossover
            low_lp: LR4Filter::new(low_freq, sample_rate, false),
            mid_hp_low: LR4Filter::new(low_freq, sample_rate, true),
            // Mid/high crossover
            mid_lp_high: LR4Filter::new(high_freq, sample_rate, false),
            high_hp: LR4Filter::new(high_freq, sample_rate, true),
            // Allpass compensation for low band (matches phase delay of mid/high split)
            low_allpass_lp: LR4Filter::new(high_freq, sample_rate, false),
            low_allpass_hp: LR4Filter::new(high_freq, sample_rate, true),

            low_env: EnvelopeFollower::from_ms(1.0, 50.0, sample_rate),
            mid_env: EnvelopeFollower::from_ms(1.0, 50.0, sample_rate),
            high_env: EnvelopeFollower::from_ms(1.0, 50.0, sample_rate),

            low_dc: DcBlocker::new(15.0, sample_rate),
            mid_dc: DcBlocker::new(15.0, sample_rate),
            high_dc: DcBlocker::new(15.0, sample_rate),

            low_drive: low_drive.clamp(0.0, 1.0),
            mid_drive: mid_drive.clamp(0.0, 1.0),
            high_drive: high_drive.clamp(0.0, 1.0),
            low_level: low_level.clamp(0.0, 2.0),
            mid_level: mid_level.clamp(0.0, 2.0),
            high_level: high_level.clamp(0.0, 2.0),
            low_freq: low_freq.clamp(50.0, 500.0),
            high_freq: high_freq.clamp(1000.0, 6000.0),

            sample_rate,
        }
    }

    fn update_crossover_frequencies(&mut self) {
        self.low_lp.set_cutoff(self.low_freq, self.sample_rate);
        self.mid_hp_low.set_cutoff(self.low_freq, self.sample_rate);
        self.mid_lp_high
            .set_cutoff(self.high_freq, self.sample_rate);
        self.high_hp.set_cutoff(self.high_freq, self.sample_rate);
        self.low_allpass_lp
            .set_cutoff(self.high_freq, self.sample_rate);
        self.low_allpass_hp
            .set_cutoff(self.high_freq, self.sample_rate);
    }
}

impl Stage for MultibandSaturatorStage {
    fn process(&mut self, input: f32) -> f32 {
        // Split into three bands using LR4 crossovers
        // First split: low vs high_side at low_freq
        let low_raw = self.low_lp.process(input);
        let high_side = self.mid_hp_low.process(input);

        // Second split: mid vs high from high_side at high_freq
        let mid = self.mid_lp_high.process(high_side);
        let high = self.high_hp.process(high_side);

        // Allpass phase compensation for low band (LP + HP at high_freq = allpass)
        let low = self.low_allpass_lp.process(low_raw) + self.low_allpass_hp.process(low_raw);

        // Track envelopes for adaptive saturation
        let low_env = self.low_env.process(low);
        let mid_env = self.mid_env.process(mid);
        let high_env = self.high_env.process(high);

        // Apply saturation with envelope-based gain compensation
        // This helps maintain consistent apparent loudness
        let low_sat = if low_env > 0.0001 {
            saturate(low / (1.0 + low_env), self.low_drive) * (1.0 + low_env * 0.5)
        } else {
            saturate(low, self.low_drive)
        };

        let mid_sat = if mid_env > 0.0001 {
            saturate(mid / (1.0 + mid_env), self.mid_drive) * (1.0 + mid_env * 0.5)
        } else {
            saturate(mid, self.mid_drive)
        };

        let high_sat = if high_env > 0.0001 {
            saturate(high / (1.0 + high_env), self.high_drive) * (1.0 + high_env * 0.5)
        } else {
            saturate(high, self.high_drive)
        };

        // Apply DC blocking to remove any DC offset from saturation
        let low_clean = self.low_dc.process(low_sat);
        let mid_clean = self.mid_dc.process(mid_sat);
        let high_clean = self.high_dc.process(high_sat);

        // Mix bands with level controls and sum
        low_clean * self.low_level + mid_clean * self.mid_level + high_clean * self.high_level
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
                if (50.0..=500.0).contains(&value) {
                    self.low_freq = value;
                    self.update_crossover_frequencies();
                    Ok(())
                } else {
                    Err("Low freq must be 50-500 Hz")
                }
            }
            "high_freq" => {
                if (1000.0..=6000.0).contains(&value) {
                    self.high_freq = value;
                    self.update_crossover_frequencies();
                    Ok(())
                } else {
                    Err("High freq must be 1000-6000 Hz")
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
    fn test_saturation_function() {
        // Clean signal (drive = 0)
        let clean = saturate(0.5, 0.0);
        // drive=0 → drive_scaled=1.0, x=0.5, output = 0.5/1.5 ≈ 0.333
        assert!((clean - 1.0 / 3.0).abs() < 0.01);

        // Heavy saturation (drive = 1)
        let saturated = saturate(0.5, 1.0);
        // drive=1 → drive_scaled=10.0, x=5.0, output = 5.0/6.0 ≈ 0.833
        assert!(saturated > 0.0);
        assert!(saturated < 1.0); // Bounded waveshaper stays below 1.0

        // Negative values
        let neg = saturate(-0.5, 0.5);
        assert!(neg < 0.0);
        assert!(neg > -1.0); // Bounded below too
    }

    #[test]
    fn test_waveshaper_bounded() {
        // Verify saturate() output is always in (-1, 1) for extreme inputs
        for &drive in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            for &input in &[0.0, 0.1, 0.5, 1.0, 5.0, 100.0, 10000.0] {
                let pos = saturate(input, drive);
                let neg = saturate(-input, drive);
                assert!(
                    pos.abs() < 1.0,
                    "saturate({input}, {drive}) = {pos}, expected |output| < 1.0"
                );
                assert!(
                    neg.abs() < 1.0,
                    "saturate({}, {drive}) = {neg}, expected |output| < 1.0",
                    -input
                );
                // Odd symmetry: f(-x) = -f(x)
                assert!(
                    (pos + neg).abs() < 1e-6,
                    "saturate is not odd-symmetric for input={input}, drive={drive}"
                );
            }
        }
    }

    #[test]
    fn test_crossover_reconstruction() {
        // With all drives at 0.0 and levels at 1.0, the crossover should preserve
        // signal energy (flat magnitude response). The LR4 crossover introduces
        // group delay (allpass), so we compare RMS energy rather than sample-by-sample.
        // Small amplitudes keep the saturator in its linear region.
        let mut stage = MultibandSaturatorStage::new(
            0.0, 0.0, 0.0, // drives = 0
            1.0, 1.0, 1.0, // levels
            200.0, 2000.0, // crossover frequencies
            48000.0,
        );

        let num_samples = 48000; // 1 second
        let settle = 24000; // let filters reach steady state
        let mut input_rms_sum: f64 = 0.0;
        let mut output_rms_sum: f64 = 0.0;
        let mut measurement_samples = 0u64;

        for i in 0..num_samples {
            let t = i as f32 / 48000.0;
            // Mix of frequencies across all bands, small amplitude
            let input = 0.00001 * (2.0 * PI * 100.0 * t).sin() // low band
                      + 0.00001 * (2.0 * PI * 800.0 * t).sin() // mid band
                      + 0.00001 * (2.0 * PI * 4000.0 * t).sin(); // high band

            let output = stage.process(input);

            if i >= settle {
                input_rms_sum += (input as f64) * (input as f64);
                output_rms_sum += (output as f64) * (output as f64);
                measurement_samples += 1;
            }
        }

        let input_rms = (input_rms_sum / measurement_samples as f64).sqrt();
        let output_rms = (output_rms_sum / measurement_samples as f64).sqrt();

        // The output RMS should be very close to input RMS (flat magnitude response).
        // Allow 1% deviation for floating-point and DC blocker effects.
        let ratio = output_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "Crossover reconstruction energy mismatch: output/input RMS ratio = {ratio}"
        );
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
}
