use crate::sim::stages::level::LevelStage;

#[derive(Debug, Clone, Copy)]
pub struct LevelConfig {
    pub gain: f32,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl LevelConfig {
    pub fn to_stage(&self, name: &str) -> LevelStage {
        LevelStage::new(name, self.gain)
    }
}
