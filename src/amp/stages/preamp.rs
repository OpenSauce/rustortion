use crate::amp::stages::Stage;
use crate::amp::stages::clipper::ClipperType;
use crate::amp::stages::common::{DcBlocker, OnePoleLP};

pub struct PreampStage {
    gain: f32, // 0..10
    bias: f32, // −1..+1
    clipper_type: ClipperType,
    interstage_lp: OnePoleLP,
    dc_blocker: DcBlocker,
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType, sample_rate: f32) -> Self {
        Self {
            gain,
            bias: bias.clamp(-1.0, 1.0),
            clipper_type: clipper,
            interstage_lp: OnePoleLP::new(10_000.0, sample_rate),
            dc_blocker: DcBlocker::new(15.0, sample_rate),
        }
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        const DRIVE_MIN: f32 = 1.0;
        const DRIVE_SCALE: f32 = 1.8;
        const CLIPPER_SCALE: f32 = 0.3;

        let drive = self.gain.mul_add(DRIVE_SCALE, DRIVE_MIN);

        // --- Initial asymmetric soft clip with DC compensation ---
        // Instead of adding DC to the input, shift the tanh curve and recenter:
        let pre = drive.mul_add(input, self.bias).tanh() - self.bias.tanh();

        // Inter-stage lowpass: models plate load capacitance rolling off upper
        // harmonics before they reach the next nonlinearity. Without this,
        // cascaded waveshapers re-distort the full harmonic spectrum, producing fizz.
        let filtered = self.interstage_lp.process(pre);

        // Main clipper expects roughly zero-centered signal; keep threshold tied to gain
        let clipped = self
            .clipper_type
            .process(filtered, self.gain.mul_add(CLIPPER_SCALE, 1.0));

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
