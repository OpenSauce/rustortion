use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum DistortionMode {
    Tanh,
    HardClip,
    Asymmetric,
    Sigmoid,
    ArcTan,
    Polynomial,
    DiodeLike,
    WaveFold,
}

pub struct Amp {
    gain: f32,
    drive: f32,
    gate_threshold: f32,
    lowpass_alpha: f32,
    lowpass_prev: f32,
    highpass_alpha: f32,
    highpass_prev: f32,
    distorted_prev: f32,
    mode: DistortionMode,
    level: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AmpConfig {
    pub gain: f32,
    pub drive: f32,
    pub gate_threshold: f32,
    pub lowpass_cutoff: f32,
    pub highpass_cutoff: f32,
    pub mode: DistortionMode,
    pub level: f32,
}

impl Amp {
    pub fn new(config: AmpConfig, sample_rate: f32) -> Self {
        let AmpConfig {
            gain,
            drive,
            gate_threshold,
            lowpass_cutoff,
            highpass_cutoff,
            mode,
            level,
        } = config;

        let rc_lp = 1.0 / (2.0 * PI * lowpass_cutoff);
        let alpha_lp = (1.0 / sample_rate) / (rc_lp + (1.0 / sample_rate));

        let rc_hp = 1.0 / (2.0 * PI * highpass_cutoff);
        let alpha_hp = rc_hp / (rc_hp + (1.0 / sample_rate));

        Self {
            gain,
            drive,
            gate_threshold,
            lowpass_alpha: alpha_lp,
            highpass_alpha: alpha_hp,
            lowpass_prev: 0.0,
            highpass_prev: 0.0,
            distorted_prev: 0.0,
            mode,
            level,
        }
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let preamp = input * self.gain * self.drive;
        let gated = if preamp.abs() > self.gate_threshold {
            preamp
        } else {
            0.0
        };

        let distorted = match self.mode {
            DistortionMode::Tanh => gated.tanh(),
            DistortionMode::HardClip => gated.clamp(-1.0, 1.0),
            DistortionMode::Asymmetric => gated.tanh() + 0.3 * gated,
            DistortionMode::Sigmoid => gated / (1.0 + gated.abs()),

            DistortionMode::ArcTan => gated.atan() * (2.0 / PI),

            DistortionMode::Polynomial => {
                let x = gated;
                let poly = (3.0 * x - x.powi(3)) * 0.5;
                poly.clamp(-1.0, 1.0)
            }

            DistortionMode::DiodeLike => {
                if gated >= 0.0 {
                    gated / (1.0 + gated.abs())
                } else {
                    0.3 * gated
                }
            }

            DistortionMode::WaveFold => foldback(gated, 1.0),
        };

        let highpassed =
            self.highpass_alpha * (self.highpass_prev + distorted - self.distorted_prev);
        self.distorted_prev = distorted;
        self.highpass_prev = highpassed;

        let filtered = self.lowpass_prev + self.lowpass_alpha * (highpassed - self.lowpass_prev);
        self.lowpass_prev = filtered;

        filtered * self.level
    }
}

fn foldback(mut x: f32, limit: f32) -> f32 {
    while x > limit {
        x = 2.0 * limit - x;
    }
    while x < -limit {
        x = -2.0 * limit - x;
    }
    x
}
