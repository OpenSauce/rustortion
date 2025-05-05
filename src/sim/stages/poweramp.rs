use crate::sim::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum PowerAmpType {
    ClassA,
    ClassAB,
    ClassB,
}

pub struct PowerAmpStage {
    name: String,
    drive: f32,
    amp_type: PowerAmpType,
    sag: f32,
    sag_envelope: f32,
    sample_rate: f32,
}

impl PowerAmpStage {
    pub fn new(name: &str, drive: f32, amp_type: PowerAmpType, sag: f32, sample_rate: f32) -> Self {
        Self {
            name: name.to_string(),
            drive: drive.clamp(0.0, 1.0),
            amp_type,
            sag: sag.clamp(0.0, 1.0),
            sag_envelope: 0.0,
            sample_rate,
        }
    }
}

impl Stage for PowerAmpStage {
    fn process(&mut self, input: f32) -> f32 {
        // Calculate sag effect (voltage dropping under load)
        // This creates dynamic compression and affects the frequency response
        let input_abs = input.abs();

        // Sag envelope follower
        let sag_attack = (-1.0 / (self.sample_rate * 0.005)).exp(); // 5ms attack
        let sag_release = (-1.0 / (self.sample_rate * 0.050)).exp(); // 50ms release

        if input_abs > self.sag_envelope {
            self.sag_envelope = sag_attack * (self.sag_envelope - input_abs) + input_abs;
        } else {
            self.sag_envelope = sag_release * (self.sag_envelope - input_abs) + input_abs;
        }

        // Calculate dynamic drive reduction from sag
        let sag_amount = 1.0 - (self.sag * self.sag_envelope * 0.5).min(0.5);
        let dynamic_drive = self.drive * sag_amount;

        // Apply power amp clipping based on type
        let driven = input * (1.0 + dynamic_drive * 3.0);

        match self.amp_type {
            PowerAmpType::ClassA => {
                // Class A - smooth, asymmetric clipping
                if driven > 0.0 {
                    driven.tanh()
                } else {
                    driven * 0.8 // Less gain for negative side
                }
            }
            PowerAmpType::ClassAB => {
                // Class AB - asymmetric with crossover characteristics
                if driven > 0.15 {
                    // Positive values above crossover
                    driven.tanh()
                } else if driven < -0.15 {
                    // Negative values below crossover
                    0.9 * driven.tanh() // Slightly less gain on negative
                } else {
                    // Crossover region with slight distortion
                    driven * (1.0 + 0.2 * driven.abs())
                }
            }
            PowerAmpType::ClassB => {
                // Class B - hard crossover distortion
                if driven > 0.0 {
                    driven.tanh()
                } else {
                    driven * 0.9 // Less gain on negative side creating crossover distortion
                }
            }
        }
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "drive" => {
                if value >= 0.0 && value <= 1.0 {
                    self.drive = value;
                    Ok(())
                } else {
                    Err("Drive must be between 0.0 and 1.0")
                }
            }
            "sag" => {
                if value >= 0.0 && value <= 1.0 {
                    self.sag = value;
                    Ok(())
                } else {
                    Err("Sag must be between 0.0 and 1.0")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "drive" => Ok(self.drive),
            "sag" => Ok(self.sag),
            _ => Err("Unknown parameter name"),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
