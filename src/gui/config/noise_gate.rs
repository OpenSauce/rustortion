use crate::sim::stages::noise_gate::NoiseGateStage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NoiseGateConfig {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
}

impl Default for NoiseGateConfig {
    fn default() -> Self {
        Self {
            threshold_db: -40.0, // Typical threshold for guitar
            ratio: 10.0,         // 10:1 ratio for smooth gating
            attack_ms: 1.0,      // Fast attack to not cut transients
            hold_ms: 10.0,       // Small hold to avoid choppy gating
            release_ms: 100.0,   // Smooth release
        }
    }
}

impl NoiseGateConfig {
    pub fn to_stage(&self, name: &str, sample_rate: f32) -> NoiseGateStage {
        NoiseGateStage::new(
            name,
            self.threshold_db,
            self.ratio,
            self.attack_ms,
            self.hold_ms,
            self.release_ms,
            sample_rate,
        )
    }
}
