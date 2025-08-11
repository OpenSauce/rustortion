use crate::sim::stages::Stage;
use crate::sim::stages::clipper::ClipperType;

pub struct PreampStage {
    gain: f32, // 0..10 → 0..+20 dB roughly
    bias: f32, // −1..+1  (≈ ±1 V)
    clipper_type: ClipperType,
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType) -> Self {
        Self {
            gain,
            bias: bias.clamp(-1.0, 1.0),
            clipper_type: clipper,
        }
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        let biased = input + self.bias;
        let amp = biased * (1.0 + self.gain * 1.8); // 0..10 → ×1..×19
        // Mild soft‑clip before main clipper to tame spikes
        let amp = (amp * 0.5).tanh() * 2.0;
        // Main clipper
        let out = self.clipper_type.process(amp, 1.0 + self.gain * 0.3);
        out * 0.8 // leave headroom for next stage
    }

    fn set_parameter(&mut self, p: &str, v: f32) -> Result<(), &'static str> {
        match p {
            "gain" => {
                if (0.0..=10.0).contains(&v) {
                    self.gain = v;
                    Ok(())
                } else {
                    Err("Gain 0‑10")
                }
            }
            "bias" => {
                if (-1.0..=1.0).contains(&v) {
                    self.bias = v;
                    Ok(())
                } else {
                    Err("Bias −1‑1")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, p: &str) -> Result<f32, &'static str> {
        match p {
            "gain" => Ok(self.gain),
            "bias" => Ok(self.bias),
            _ => Err("Unknown parameter"),
        }
    }
}
