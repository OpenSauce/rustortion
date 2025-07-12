use crate::sim::stages::poweramp::{PowerAmpStage, PowerAmpType};

#[derive(Debug, Clone, Copy)]
pub struct PowerAmpConfig {
    pub drive: f32,
    pub amp_type: PowerAmpType,
    pub sag: f32,
}

impl Default for PowerAmpConfig {
    fn default() -> Self {
        Self {
            drive: 0.5,
            amp_type: PowerAmpType::ClassAB,
            sag: 0.3,
        }
    }
}

impl PowerAmpConfig {
    pub fn to_stage(&self, name: &str, sample_rate: f32) -> PowerAmpStage {
        PowerAmpStage::new(name, self.drive, self.amp_type, self.sag, sample_rate)
    }
}
