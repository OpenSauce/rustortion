use crate::amp::stages::poweramp::{PowerAmpStage, PowerAmpType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    pub fn to_stage(&self, sample_rate: f32) -> PowerAmpStage {
        PowerAmpStage::new(self.drive, self.amp_type, self.sag, sample_rate)
    }
}
