use crate::sim::stages::Stage;
use crate::sim::stages::clipper::ClipperType;

pub struct PreampStage {
    gain: f32, // 0..10
    bias: f32, // −1..+1
    clipper_type: ClipperType,

    // DC blocker state
    dc_x1: f32,
    dc_y1: f32,
    dc_r: f32, // computed from fs/fc
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType) -> Self {
        // choose a very low cutoff; tweak as you like
        let fs = 48_000.0;
        let fc = 5.0;
        let dc_r = (-2.0 * std::f32::consts::PI * fc / fs).exp();

        Self {
            gain,
            bias: bias.clamp(-1.0, 1.0),
            clipper_type: clipper,
            dc_x1: 0.0,
            dc_y1: 0.0,
            dc_r,
        }
    }

    #[inline]
    fn dc_block(&mut self, x: f32) -> f32 {
        // y[n] = x[n] - x[n-1] + R*y[n-1]
        let y = x - self.dc_x1 + self.dc_r * self.dc_y1;
        self.dc_x1 = x;
        self.dc_y1 = y;
        y
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        // Drive mapping: 0..10 → ~1..19
        let drive = 1.0 + self.gain * 1.8;
        let b = self.bias;

        // --- Asymmetric soft clip with DC compensation ---
        // Instead of adding DC to the input, shift the tanh curve and recenter:
        let pre = (drive * input + b).tanh() - b.tanh();

        // Optional *gentle* pre-tamer (keep if you like the feel):
        // let pre = (pre * 0.5).tanh() * 2.0;

        // Main clipper expects roughly zero-centered signal; keep threshold tied to gain
        let clipped = self.clipper_type.process(pre, 1.0 + self.gain * 0.3);

        // Remove any residual DC so next stage gets a clean, centered signal
        let cleaned = self.dc_block(clipped);

        cleaned * 0.8 // headroom
    }

    fn set_parameter(&mut self, p: &str, v: f32) -> Result<(), &'static str> {
        match p {
            "gain" => {
                if (0.0..=10.0).contains(&v) {
                    self.gain = v;
                    Ok(())
                } else {
                    Err("Gain 0-10")
                }
            }
            "bias" => {
                if (-1.0..=1.0).contains(&v) {
                    self.bias = v;
                    Ok(())
                } else {
                    Err("Bias −1-1")
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
