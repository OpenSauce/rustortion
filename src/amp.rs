use clap::ValueEnum;
use std::f32::consts::PI;
use std::fmt;
#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum DistortionMode {
    Tanh,
    HardClip,
    Asymmetric,
    Sigmoid,
}

impl fmt::Display for DistortionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DistortionMode::Tanh => "tanh",
            DistortionMode::HardClip => "hardclip",
            DistortionMode::Asymmetric => "asymmetric",
            DistortionMode::Sigmoid => "sigmoid",
        };
        write!(f, "{s}")
    }
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
}

impl Amp {
    pub fn new(gain: f32, sample_rate: f32, mode: DistortionMode) -> Self {
        let lowpass_cutoff = 6000.0;
        let highpass_cutoff = 120.0;

        let rc_lp = 1.0 / (2.0 * PI * lowpass_cutoff);
        let alpha_lp = (1.0 / sample_rate) / (rc_lp + (1.0 / sample_rate));

        let rc_hp = 1.0 / (2.0 * PI * highpass_cutoff);
        let alpha_hp = rc_hp / (rc_hp + (1.0 / sample_rate));

        Self {
            gain,
            drive: 50.0,
            gate_threshold: 0.02,
            lowpass_alpha: alpha_lp,
            lowpass_prev: 0.0,
            highpass_alpha: alpha_hp,
            highpass_prev: 0.0,
            distorted_prev: 0.0,
            mode,
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
        };

        let highpassed =
            self.highpass_alpha * (self.highpass_prev + distorted - self.distorted_prev);
        self.distorted_prev = distorted;
        self.highpass_prev = highpassed;

        let filtered = self.lowpass_prev + self.lowpass_alpha * (highpassed - self.lowpass_prev);
        self.lowpass_prev = filtered;

        filtered
    }
}
