use crate::sim::stages::{clipper::ClipperType, preamp::PreampStage};

#[derive(Debug, Clone, Copy)]
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
    pub fn to_stage(&self, name: &str) -> PreampStage {
        PreampStage::new(name, self.gain, self.bias, self.clipper_type)
    }
}
