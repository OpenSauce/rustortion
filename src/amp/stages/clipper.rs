use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
            Self::Soft => write!(f, "{}", crate::tr!(clipper_soft)),
            Self::Medium => write!(f, "{}", crate::tr!(clipper_medium)),
            Self::Hard => write!(f, "{}", crate::tr!(clipper_hard)),
            Self::Asymmetric => write!(f, "{}", crate::tr!(clipper_asymmetric)),
            Self::ClassA => write!(f, "{}", crate::tr!(clipper_class_a)),
        }
    }
}

impl ClipperType {
    pub fn process(&self, input: f32, drive: f32) -> f32 {
        let driven = input * drive;

        match self {
            Self::Soft => {
                // Soft clipping using tanh for smooth tube-like saturation
                driven.tanh()
            }

            Self::Medium => {
                // Medium clipping using arctan for a balanced distortion
                driven.atan() * (2.0 / PI)
            }

            Self::Hard => {
                // Hard clipping with sharp cutoff
                driven.clamp(-1.0, 1.0)
            }

            Self::Asymmetric => {
                // Asymmetric clipping to model even harmonics from tubes
                // Positive signals clip differently than negative ones
                if driven >= 0.0 {
                    driven.tanh()
                } else {
                    0.7f32.mul_add(driven.tanh(), 0.3 * driven)
                }
            }

            Self::ClassA => {
                // Class A tube preamp behavior
                // Combines soft clipping with subtle wave folding for complex harmonics
                let soft_clip = driven.tanh();
                let fold_amount: f32 = 0.3;
                let folded = if driven.abs() > 1.0 {
                    let fold_factor = 2.0 - driven.abs().min(2.0);
                    soft_clip * fold_factor
                } else {
                    soft_clip
                };

                (1.0 - fold_amount).mul_add(soft_clip, fold_amount * folded)
            }
        }
    }
}
