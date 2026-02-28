use crate::amp::stages::Stage;
use crate::amp::stages::common::calculate_coefficient;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PowerAmpType {
    ClassA,
    ClassAB,
    ClassB,
}

impl std::fmt::Display for PowerAmpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClassA => write!(f, "{}", crate::tr!(poweramp_class_a)),
            Self::ClassAB => write!(f, "{}", crate::tr!(poweramp_class_ab)),
            Self::ClassB => write!(f, "{}", crate::tr!(poweramp_class_b)),
        }
    }
}

pub struct PowerAmpStage {
    drive: f32,
    amp_type: PowerAmpType,
    sag: f32,
    sag_envelope: f32,
    sag_attack_coeff: f32,
    sag_release_coeff: f32,
}

impl PowerAmpStage {
    pub fn new(drive: f32, amp_type: PowerAmpType, sag: f32, sample_rate: f32) -> Self {
        Self {
            drive: drive.clamp(0.0, 1.0),
            amp_type,
            sag: sag.clamp(0.0, 1.0),
            sag_envelope: 0.0,
            sag_attack_coeff: calculate_coefficient(2.0, sample_rate), // 2ms attack
            sag_release_coeff: calculate_coefficient(50.0, sample_rate), // 50ms release
        }
    }
}

impl Stage for PowerAmpStage {
    fn process(&mut self, input: f32) -> f32 {
        // Calculate dynamic drive reduction from sag
        let sag_amount = 1.0 - (self.sag * self.sag_envelope * 0.5).min(0.5);
        let dynamic_drive = self.drive * sag_amount;

        // Apply power amp drive
        let driven = input * (1.0 + dynamic_drive * 3.0);

        // Update sag envelope from post-drive signal for consistent behavior
        let driven_abs = driven.abs();
        if driven_abs > self.sag_envelope {
            self.sag_envelope = self
                .sag_attack_coeff
                .mul_add(self.sag_envelope - driven_abs, driven_abs);
        } else {
            self.sag_envelope = self
                .sag_release_coeff
                .mul_add(self.sag_envelope - driven_abs, driven_abs);
        }
        // Denormal protection
        if self.sag_envelope.abs() < 1e-20 {
            self.sag_envelope = 0.0;
        }

        match self.amp_type {
            PowerAmpType::ClassA => {
                // Class A: smooth, asymmetric saturation.
                // Both halves bounded via tanh; negative side has reduced gain
                // for the even-harmonic asymmetry characteristic of single-ended amps.
                if driven >= 0.0 {
                    driven.tanh()
                } else {
                    (driven * 0.8).tanh()
                }
            }
            PowerAmpType::ClassAB => {
                // Class AB: smooth deadzone around zero for crossover distortion,
                // then asymmetric saturation on both halves.
                // f(x) = x³/(x²+dz²) is C∞ smooth, gain→0 at zero crossing,
                // gain→1 for |x|>>dz — models reduced transconductance near zero.
                let dz: f32 = 0.1;
                let x2 = driven * driven;
                let crossover = driven * x2 / (dz.mul_add(dz, x2));
                if crossover >= 0.0 {
                    crossover.tanh()
                } else {
                    (crossover * 0.9).tanh()
                }
            }
            PowerAmpType::ClassB => {
                // Class B: push-pull with stronger asymmetry than Class AB.
                // Both halves bounded via tanh; negative side has reduced gain
                // creating the characteristic crossover distortion.
                if driven >= 0.0 {
                    driven.tanh()
                } else {
                    (driven * 0.7).tanh()
                }
            }
        }
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "drive" => {
                if (0.0..=1.0).contains(&value) {
                    self.drive = value;
                    Ok(())
                } else {
                    Err("Drive must be between 0.0 and 1.0")
                }
            }
            "sag" => {
                if (0.0..=1.0).contains(&value) {
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
}
