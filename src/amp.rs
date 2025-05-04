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
    compressor_env: f32,
    comp_attack: f32,    // 0.5 ms
    comp_release: f32,   // 100 ms
    comp_threshold: f32, // –20 dB
    comp_ratio: f32,     // 4:1
    makeup: f32,         // +6 dB
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

    #[serde(default = "default_attack_ms")]
    pub comp_attack_ms: f32, // 0.5
    #[serde(default = "default_release_ms")]
    pub comp_release_ms: f32, // 100
    #[serde(default = "default_threshold_db")]
    pub comp_threshold_db: f32, // -20
    #[serde(default = "default_ratio")]
    pub comp_ratio: f32, // 4.0
    #[serde(default = "default_makeup_db")]
    pub makeup_db: f32,
}

fn default_attack_ms() -> f32 {
    0.5
}
fn default_release_ms() -> f32 {
    100.0
}
fn default_threshold_db() -> f32 {
    -20.0
}
fn default_ratio() -> f32 {
    4.0
}
fn default_makeup_db() -> f32 {
    6.0
}
#[inline]
fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl Amp {
    pub fn new(config: AmpConfig, sample_rate: f32) -> Self {
        let rc_lp = 1.0 / (2.0 * PI * config.lowpass_cutoff);
        let alpha_lp = (1.0 / sample_rate) / (rc_lp + (1.0 / sample_rate));

        let rc_hp = 1.0 / (2.0 * PI * config.highpass_cutoff);
        let alpha_hp = rc_hp / (rc_hp + (1.0 / sample_rate));

        let (comp_attack, comp_release) = {
            // convert ms to one‑pole coefficients  α = e^(−1/τ)
            let a = (-1.0 / (sample_rate * 0.001 * config.comp_attack_ms)).exp();
            let r = (-1.0 / (sample_rate * 0.001 * config.comp_release_ms)).exp();
            (a, r)
        };
        Self {
            gain: config.gain,
            drive: config.drive,
            gate_threshold: config.gate_threshold,
            lowpass_alpha: alpha_lp,
            highpass_alpha: alpha_hp,
            lowpass_prev: 0.0,
            highpass_prev: 0.0,
            distorted_prev: 0.0,
            mode: config.mode,
            level: config.level,
            compressor_env: 0.0,
            comp_attack,
            comp_release,
            comp_threshold: db_to_lin(config.comp_threshold_db),
            comp_ratio: config.comp_ratio,
            makeup: db_to_lin(config.makeup_db),
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

        let level_in = distorted.abs().max(1e-10); // avoid log(0)
        if level_in > self.compressor_env {
            self.compressor_env = self.comp_attack * (self.compressor_env - level_in) + level_in;
        } else {
            self.compressor_env = self.comp_release * (self.compressor_env - level_in) + level_in;
        }

        let over_threshold = (self.compressor_env / self.comp_threshold).max(1.0);
        let gain_lin = if over_threshold > 1.0 {
            // G = (in/threshold)^(1/ratio‑1)
            over_threshold.powf((1.0 / self.comp_ratio) - 1.0) * self.makeup
        } else {
            self.makeup
        };
        let compressed = distorted * gain_lin;
        let highpassed =
            self.highpass_alpha * (self.highpass_prev + compressed - self.distorted_prev);
        self.distorted_prev = compressed;
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
