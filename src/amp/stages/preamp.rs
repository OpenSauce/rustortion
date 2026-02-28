use crate::amp::stages::Stage;
use crate::amp::stages::clipper::ClipperType;
use crate::amp::stages::common::DcBlocker;

pub struct PreampStage {
    gain: f32, // 0..10
    bias: f32, // −1..+1
    clipper_type: ClipperType,
    dc_blocker: DcBlocker,
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType, sample_rate: f32) -> Self {
        Self {
            gain,
            bias: bias.clamp(-1.0, 1.0),
            clipper_type: clipper,
            dc_blocker: DcBlocker::new(15.0, sample_rate),
        }
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
        self.dc_blocker.process(clipped)
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
