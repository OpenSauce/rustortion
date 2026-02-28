use crate::amp::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilterType {
    Highpass,
    Lowpass,
}

impl std::fmt::Display for FilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Highpass => write!(f, "{}", crate::tr!(filter_highpass)),
            Self::Lowpass => write!(f, "{}", crate::tr!(filter_lowpass)),
        }
    }
}

pub struct FilterStage {
    filter_type: FilterType,
    cutoff: f32,
    alpha: f32,
    prev_input: f32,
    prev_output: f32,
    sample_rate: f32,
}

impl FilterStage {
    /// Minimum cutoff frequency to avoid division-by-zero in the RC calculation.
    /// 0.1 Hz makes a highpass effectively a passthrough.
    const MIN_CUTOFF_HZ: f32 = 0.1;

    fn compute_alpha(filter_type: FilterType, cutoff: f32, sample_rate: f32) -> f32 {
        let rc = 1.0 / (2.0 * PI * cutoff.max(Self::MIN_CUTOFF_HZ));
        let dt = 1.0 / sample_rate;
        match filter_type {
            FilterType::Highpass => rc / (rc + dt),
            FilterType::Lowpass => dt / (rc + dt),
        }
    }

    pub fn new(filter_type: FilterType, cutoff: f32, sample_rate: f32) -> Self {
        let alpha = Self::compute_alpha(filter_type, cutoff, sample_rate);

        Self {
            filter_type,
            cutoff,
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
            sample_rate,
        }
    }

    // This recalculates alpha when cutoff changes
    fn update_coefficients(&mut self) {
        self.alpha = Self::compute_alpha(self.filter_type, self.cutoff, self.sample_rate);
    }
}

impl Stage for FilterStage {
    fn process(&mut self, input: f32) -> f32 {
        match self.filter_type {
            FilterType::Highpass => {
                // First-order highpass filter
                let output = self.alpha * (self.prev_output + input - self.prev_input);
                self.prev_input = input;
                self.prev_output = output;
                output
            }
            FilterType::Lowpass => {
                // First-order lowpass filter
                let output = self
                    .alpha
                    .mul_add(input - self.prev_output, self.prev_output);
                self.prev_output = output;
                output
            }
        }
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "cutoff" => {
                if (0.0..=20000.0).contains(&value) {
                    self.cutoff = value;
                    self.update_coefficients();
                    Ok(())
                } else {
                    Err("Cutoff must be between 0Hz and 20kHz")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "cutoff" => Ok(self.cutoff),
            _ => Err("Unknown parameter name"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amp::stages::Stage;

    #[test]
    fn highpass_blocks_low_frequencies_and_passes_high() {
        let sr = 48_000.0;
        let cutoff = 1_000.0;

        // ---------- DC rejection ----------
        let mut hp = FilterStage::new(FilterType::Highpass, cutoff, sr);

        // Warm up on DC to let the step transient decay.
        for _ in 0..512 {
            hp.process(1.0);
        }

        // Now measure on DC after warm-up.
        let mut dc_sum = 0.0;
        let n = 256;
        for _ in 0..n {
            dc_sum += hp.process(1.0);
        }
        let dc_avg = dc_sum / n as f32;
        assert!(
            dc_avg.abs() < 1e-3,
            "DC not attenuated enough after warm-up: avg={dc_avg}"
        );

        // ---------- High-frequency passthrough ----------
        // Recreate to reset state.
        let mut hp = FilterStage::new(FilterType::Highpass, cutoff, sr);

        // Use a very high-frequency square (~24 kHz): alternating +1/-1.
        // Warm up a little (filters have transients even with HF).
        for i in 0..64 {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            hp.process(s);
        }

        let mut acc = 0.0;
        for i in 0..256 {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            acc += hp.process(s).abs();
        }
        let hf_avg_abs = acc / 256.0;

        // Should be largely passed (don’t demand unity; first-order HPF still shapes HF).
        assert!(
            hf_avg_abs > 0.5,
            "High-frequency attenuated too much: avg_abs={hf_avg_abs}"
        );
    }

    #[test]
    fn highpass_zero_cutoff_produces_finite_output() {
        let mut hp = FilterStage::new(FilterType::Highpass, 0.0, 48_000.0);
        for _ in 0..256 {
            let out = hp.process(1.0);
            assert!(
                out.is_finite(),
                "zero-cutoff highpass produced non-finite output"
            );
        }
    }
}
