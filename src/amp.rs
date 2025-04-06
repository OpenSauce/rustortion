use std::f32::consts::PI;

pub struct Amp {
    gain: f32,
    drive: f32,
    gate_threshold: f32,
    lowpass_alpha: f32,
    lowpass_prev: f32,
    highpass_alpha: f32,
    highpass_prev: f32,
}

impl Amp {
    pub fn new(gain: f32, sample_rate: f32) -> Self {
        let lowpass_cutoff = 7000.0;
        let highpass_cutoff = 100.0;

        let rc_lp = 1.0 / (2.0 * PI * lowpass_cutoff);
        let alpha_lp = (1.0 / sample_rate) / (rc_lp + (1.0 / sample_rate));

        let rc_hp = 1.0 / (2.0 * PI * highpass_cutoff);
        let alpha_hp = rc_hp / (rc_hp + (1.0 / sample_rate));

        Self {
            gain,
            drive: 6.0,
            gate_threshold: 0.02,
            lowpass_alpha: alpha_lp,
            lowpass_prev: 0.0,
            highpass_alpha: alpha_hp,
            highpass_prev: 0.0,
        }
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let preamp = input * self.gain * self.drive;
        let gated = if preamp.abs() > self.gate_threshold {
            preamp
        } else {
            0.0
        };
        let distorted = gated.tanh();
        let highpassed = self.highpass_alpha * (self.highpass_prev + distorted - input);
        self.highpass_prev = highpassed;
        let filtered = self.lowpass_prev + self.lowpass_alpha * (highpassed - self.lowpass_prev);
        self.lowpass_prev = filtered;
        filtered
    }
}
