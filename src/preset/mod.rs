use serde::{Deserialize, Serialize};

use crate::gui::components::input_filter_control::InputFilterConfig;
use crate::gui::stages::StageConfig;

pub mod manager;

pub use manager::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub stages: Vec<StageConfig>,
    pub ir_name: Option<String>,
    #[serde(default = "default_ir_gain")]
    pub ir_gain: f32,
    #[serde(default)]
    pub pitch_shift_semitones: i32,
    #[serde(default)]
    pub input_filters: InputFilterConfig,
}

const fn default_ir_gain() -> f32 {
    0.1
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "New Preset".to_string(),
            author: None,
            description: None,
            stages: Vec::new(),
            ir_name: None,
            ir_gain: 0.1,
            pitch_shift_semitones: 0,
            input_filters: InputFilterConfig::default(),
        }
    }
}

impl Preset {
    pub const fn new(
        name: String,
        stages: Vec<StageConfig>,
        ir_name: Option<String>,
        ir_gain: f32,
        pitch_shift_semitones: i32,
        input_filters: InputFilterConfig,
    ) -> Self {
        Self {
            name,
            description: None,
            author: None,
            stages,
            ir_name,
            ir_gain,
            pitch_shift_semitones,
            input_filters,
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }
}
