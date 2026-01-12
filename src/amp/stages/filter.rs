use crate::amp::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    Highpass,
    Lowpass,
}

impl std::fmt::Display for FilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterType::Highpass => write!(f, "Highpass"),
            FilterType::Lowpass => write!(f, "Lowpass"),
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
    pub fn new(filter_type: FilterType, cutoff: f32, sample_rate: f32) -> Self {
        // Calculate initial alpha value from cutoff
        let alpha = match filter_type {
            FilterType::Highpass => {
                let rc = 1.0 / (2.0 * PI * cutoff);
                rc / (rc + (1.0 / sample_rate))
            }
            FilterType::Lowpass => {
                let rc = 1.0 / (2.0 * PI * cutoff);
                (1.0 / sample_rate) / (rc + (1.0 / sample_rate))
            }
        };

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
        self.alpha = match self.filter_type {
            FilterType::Highpass => {
                let rc = 1.0 / (2.0 * PI * self.cutoff);
                rc / (rc + (1.0 / self.sample_rate))
            }
            FilterType::Lowpass => {
                let rc = 1.0 / (2.0 * PI * self.cutoff);
                (1.0 / self.sample_rate) / (rc + (1.0 / self.sample_rate))
            }
        };
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
                let output = self.prev_output + self.alpha * (input - self.prev_output);
                self.prev_output = output;
                output
            }
        }
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "cutoff" => {
                if value > 20.0 && value < 20000.0 {
                    self.cutoff = value;
                    self.update_coefficients();
                    Ok(())
                } else {
                    Err("Cutoff must be between 20Hz and 20kHz")
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
}
