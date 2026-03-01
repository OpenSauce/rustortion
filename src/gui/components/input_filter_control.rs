use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InputFilterConfig {
    pub hp_enabled: bool,
    pub hp_cutoff: f32,
    pub lp_enabled: bool,
    pub lp_cutoff: f32,
}

impl Default for InputFilterConfig {
    fn default() -> Self {
        Self {
            hp_enabled: true,
            hp_cutoff: 100.0,
            lp_enabled: true,
            lp_cutoff: 8000.0,
        }
    }
}
