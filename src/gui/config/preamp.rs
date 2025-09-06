use crate::sim::stages::{clipper::ClipperType, preamp::PreampStage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PreampConfig {
    pub gain: f32,
    pub bias: f32,
    pub clipper_type: ClipperType,
}

impl Default for PreampConfig {
    fn default() -> Self {
        Self {
            gain: 5.0,
            bias: 0.0,
            clipper_type: ClipperType::Soft,
        }
    }
}

impl PreampConfig {
    #[must_use]
    pub fn to_stage(&self) -> PreampStage {
        PreampStage::new(self.gain, self.bias, self.clipper_type)
    }
}
