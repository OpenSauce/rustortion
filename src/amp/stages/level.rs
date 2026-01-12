use crate::amp::stages::Stage;

pub struct LevelStage {
    gain: f32,
}

impl LevelStage {
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }
}

impl Stage for LevelStage {
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

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "gain" => Ok(self.gain),
            _ => Err("Unknown parameter name"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_stage() {
        let mut stage = LevelStage::new(1.0);
        assert_eq!(stage.process(1.0), 1.0);

        stage.set_parameter("gain", 2.0).unwrap();
        assert_eq!(stage.process(1.0), 2.0);

        stage.set_parameter("gain", 0.5).unwrap();
        assert_eq!(stage.process(1.0), 0.5);

        assert!(stage.set_parameter("gain", 3.0).is_err());
    }
}
