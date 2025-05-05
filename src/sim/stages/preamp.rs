use crate::sim::stages::{Stage, clipper::ClipperType};

pub struct PreampStage {
    name: String,
    gain: f32, // Gain for this stage (0.0 to 10.0)
    bias: f32, // DC bias applied to the signal (-1.0 to 1.0)
    clipper_type: ClipperType,
}

impl PreampStage {
    pub fn new(name: &str, gain: f32, bias: f32, clipper_type: ClipperType) -> Self {
        Self {
            name: name.to_string(),
            gain,
            bias,
            clipper_type,
        }
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        // Apply bias shift
        let biased = input + self.bias;

        // Apply gain
        let amplified = biased * self.gain;

        // Process through clipper (distortion)
        self.clipper_type
            .process(amplified, 1.0 + (self.gain * 0.3))
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "gain" => {
                if value >= 0.0 && value <= 10.0 {
                    self.gain = value;
                    Ok(())
                } else {
                    Err("Gain must be between 0.0 and 10.0")
                }
            }
            "bias" => {
                if value >= -1.0 && value <= 1.0 {
                    self.bias = value;
                    Ok(())
                } else {
                    Err("Bias must be between -1.0 and 1.0")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "gain" => Ok(self.gain),
            "bias" => Ok(self.bias),
            _ => Err("Unknown parameter name"),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
