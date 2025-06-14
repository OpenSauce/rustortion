use crate::sim::stages::Stage;

pub struct LevelStage {
    name: String,
    gain: f32,
}

impl LevelStage {
    pub fn new(name: &str, gain: f32) -> Self {
        Self {
            name: name.into(),
            gain,
        }
    }
}

impl Stage for LevelStage {
    fn name(&self) -> &str {
        &self.name
    }

    fn process(&mut self, input: f32) -> f32 {
        input * self.gain
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "gain" => {
                if (0.0..=2.0).contains(&value) {
                    self.gain = value;
                    Ok(())
                } else {
                    Err("Gain must be between 0.0 and 2.0")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    // Get a parameter value by name
    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "gain" => Ok(self.gain),
            _ => Err("Unknown parameter name"),
        }
    }
}
