use crate::sim::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum FilterType {
    Highpass,
    Lowpass,
    Bandpass,
    Notch,
}

pub struct FilterStage {
    name: String,
    filter_type: FilterType,
    cutoff: f32,
    resonance: f32,
    alpha: f32,
    prev_input: f32,
    prev_output: f32,
    sample_rate: f32,
}

impl FilterStage {
    pub fn new(
        name: &str,
        filter_type: FilterType,
        cutoff: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> Self {
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
            FilterType::Bandpass | FilterType::Notch => {
                // For bandpass and notch we'd need a different calculation,
                // using a simplified version for now
                let rc = 1.0 / (2.0 * PI * cutoff);
                (1.0 / sample_rate) / (rc + (1.0 / sample_rate))
            }
        };

        Self {
            name: name.to_string(),
            filter_type,
            cutoff,
            resonance,
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
            FilterType::Bandpass | FilterType::Notch => {
                // For bandpass and notch we'd need a different calculation,
                // using a simplified version for now
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
            FilterType::Bandpass => {
                // Simple bandpass implementation
                // For a true bandpass, we would cascade highpass and lowpass
                let highpass = self.alpha * (self.prev_output + input - self.prev_input);
                let lowpass = self.prev_output + self.alpha * (highpass - self.prev_output);

                self.prev_input = input;
                self.prev_output = lowpass;

                // Apply resonance (feedback)
                lowpass * (1.0 + self.resonance * 0.9)
            }
            FilterType::Notch => {
                // Simple notch implementation
                // A true notch would require a biquad filter
                let allpass = input;
                let lowpass = self.prev_output + self.alpha * (input - self.prev_output);

                self.prev_output = lowpass;

                // Notch is all-pass minus bandpass
                allpass - (input - lowpass) * self.resonance
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
            "resonance" => {
                if value >= 0.0 && value <= 1.0 {
                    self.resonance = value;
                    Ok(())
                } else {
                    Err("Resonance must be between 0.0 and 1.0")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "cutoff" => Ok(self.cutoff),
            "resonance" => Ok(self.resonance),
            _ => Err("Unknown parameter name"),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
