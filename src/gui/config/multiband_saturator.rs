use crate::amp::stages::multiband_saturator::MultibandSaturatorStage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MultibandSaturatorConfig {
    pub low_drive: f32,
    pub mid_drive: f32,
    pub high_drive: f32,
    pub low_level: f32,
    pub mid_level: f32,
    pub high_level: f32,
    pub low_freq: f32,
    pub high_freq: f32,
}

impl Default for MultibandSaturatorConfig {
    fn default() -> Self {
        Self {
            low_drive: 0.3,
            mid_drive: 0.5,
            high_drive: 0.4,
            low_level: 1.0,
            mid_level: 1.0,
            high_level: 1.0,
            low_freq: 200.0,
            high_freq: 2500.0,
        }
    }
}

impl MultibandSaturatorConfig {
    pub fn to_stage(&self, sample_rate: f32) -> MultibandSaturatorStage {
        MultibandSaturatorStage::new(
            self.low_drive,
            self.mid_drive,
            self.high_drive,
            self.low_level,
            self.mid_level,
            self.high_level,
            self.low_freq,
            self.high_freq,
            sample_rate,
        )
    }
}
