use crate::amp::stages::level::LevelStage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LevelConfig {
    pub gain: f32,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl LevelConfig {
    pub fn to_stage(&self) -> LevelStage {
        LevelStage::new(self.gain)
    }
}
