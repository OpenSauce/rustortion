use crate::sim::stages::Stage;
use crate::sim::stages::clipper::ClipperType;

pub struct PreampStage {
    gain: f32, // 0..10
    bias: f32, // −1..+1
    clipper_type: ClipperType,

    dc_prev_input: f32,
    dc_prev_output: f32,
    dc_coeff: f32,
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType, sample_rate: f32) -> Self {
        const DC_CUTOFF_HZ: f32 = 15.0;
        let dc_coeff = (-2.0 * std::f32::consts::PI * DC_CUTOFF_HZ / sample_rate).exp();

        Self {
            gain,
            bias: bias.clamp(-1.0, 1.0),
            clipper_type: clipper,
            dc_prev_input: 0.0,
            dc_prev_output: 0.0,
            dc_coeff,
        }
    }

    #[inline]
    /// DC blocker: y[n] = x[n] - x[n-1] + R*y[n-1]
    /// https://ccrma.stanford.edu/~jos/fp/DC_Blocker.html
    fn dc_block(&mut self, input: f32) -> f32 {
        let output = input - self.dc_prev_input + self.dc_coeff * self.dc_prev_output;
        self.dc_prev_input = input;
        self.dc_prev_output = output;
        output
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        const DRIVE_MIN: f32 = 1.0;
        const DRIVE_SCALE: f32 = 1.8;
        const CLIPPER_SCALE: f32 = 0.3;

        let drive = DRIVE_MIN + self.gain * DRIVE_SCALE;

        // --- Initial asymmetric soft clip with DC compensation ---
        // Instead of adding DC to the input, shift the tanh curve and recenter:
        let pre = (drive * input + self.bias).tanh() - self.bias.tanh();

        // Main clipper expects roughly zero-centered signal; keep threshold tied to gain
        let clipped = self
            .clipper_type
            .process(pre, 1.0 + self.gain * CLIPPER_SCALE);

        // Remove any residual DC so next stage gets a clean, centered signal
        self.dc_block(clipped)
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
