use serde::{Deserialize, Serialize};

use crate::gui::config::StageConfig;

pub mod manager;

pub use manager::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub stages: Vec<StageConfig>,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "New Preset".to_string(),
            author: None,
            description: None,
            stages: Vec::new(),
        }
    }
}

impl Preset {
    pub fn new(name: String, stages: Vec<StageConfig>) -> Self {
        Self {
            name,
            description: None,
            author: None,
            stages,
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
