use crate::sim::stages::tonestack::{ToneStackModel, ToneStackStage};

#[derive(Debug, Clone, Copy)]
pub struct ToneStackConfig {
    pub model: ToneStackModel,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
}

impl Default for ToneStackConfig {
    fn default() -> Self {
        Self {
            model: ToneStackModel::Modern,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
        }
    }
}

impl ToneStackConfig {
    pub fn to_stage(&self, name: &str, sample_rate: f32) -> ToneStackStage {
        ToneStackStage::new(
            name,
            self.model,
            self.bass,
            self.mid,
            self.treble,
            self.presence,
            sample_rate,
        )
    }
}
