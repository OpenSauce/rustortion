use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ClipperType {
    Soft,       // Smooth, tube-like saturation (similar to Tanh)
    Medium,     // Balanced clipping (similar to ArcTan)
    Hard,       // More aggressive clipping (similar to HardClip)
    Asymmetric, // Tube-like even harmonic generation
    ClassA,     // Classic Class A tube preamp behavior
}

impl std::fmt::Display for ClipperType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipperType::Soft => write!(f, "Soft Clipping"),
            ClipperType::Medium => write!(f, "Medium Clipping"),
            ClipperType::Hard => write!(f, "Hard Clipping"),
            ClipperType::Asymmetric => write!(f, "Asymmetric Clipping"),
            ClipperType::ClassA => write!(f, "Class A Tube Preamp"),
        }
    }
}

impl ClipperType {
    pub fn process(&self, input: f32, drive: f32) -> f32 {
        let driven = input * drive;

        match self {
            ClipperType::Soft => {
                // Soft clipping using tanh for smooth tube-like saturation
                driven.tanh()
            }

            ClipperType::Medium => {
                // Medium clipping using arctan for a balanced distortion
                driven.atan() * (2.0 / PI)
            }

            ClipperType::Hard => {
                // Hard clipping with sharp cutoff
                driven.clamp(-1.0, 1.0)
            }

            ClipperType::Asymmetric => {
                // Asymmetric clipping to model even harmonics from tubes
                // Positive signals clip differently than negative ones
                if driven >= 0.0 {
                    driven.tanh()
                } else {
                    0.7 * driven.tanh() + 0.3 * driven
                }
            }

            ClipperType::ClassA => {
                // Class A tube preamp behavior
                // Combines soft clipping with subtle wave folding for complex harmonics
                let soft_clip = driven.tanh();
                let fold_amount = 0.3;
                let folded = if driven.abs() > 1.0 {
                    let fold_factor = 2.0 - driven.abs().min(2.0);
                    soft_clip * fold_factor
                } else {
                    soft_clip
                };

                (1.0 - fold_amount) * soft_clip + fold_amount * folded
            }
        }
    }
}
