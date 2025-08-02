use crate::sim::stages::filter::{FilterStage, FilterType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FilterConfig {
    pub filter_type: FilterType,
    pub cutoff_hz: f32,
    pub resonance: f32,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Highpass,
            cutoff_hz: 100.0,
            resonance: 0.0,
        }
    }
}

impl FilterConfig {
    pub fn to_stage(&self, name: &str, sample_rate: f32) -> FilterStage {
        FilterStage::new(
            name,
            self.filter_type,
            self.cutoff_hz,
            self.resonance,
            sample_rate,
        )
    }
}
