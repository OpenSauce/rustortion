use crate::sim::stages::compressor::CompressorStage;

#[derive(Debug, Clone, Copy)]
pub struct CompressorConfig {
    pub attack_ms: f32,
    pub release_ms: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub makeup_db: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            attack_ms: 1.0,
            release_ms: 100.0,
            threshold_db: -20.0,
            ratio: 4.0,
            makeup_db: 0.0,
        }
    }
}

impl CompressorConfig {
    pub fn to_stage(&self, name: &str, sample_rate: f32) -> CompressorStage {
        CompressorStage::new(
            name,
            self.attack_ms,
            self.release_ms,
            self.threshold_db,
            self.ratio,
            self.makeup_db,
            sample_rate,
        )
    }
}
