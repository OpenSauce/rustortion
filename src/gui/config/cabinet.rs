use crate::sim::stages::cabinet::CabinetStage;

#[derive(Debug, Clone)]
pub struct CabinetConfig {
    pub ir_path: String,
    pub enabled: bool,
}

impl Default for CabinetConfig {
    fn default() -> Self {
        Self {
            ir_path: String::new(),
            enabled: true,
        }
    }
}

impl CabinetConfig {
    pub fn to_stage(&self, name: &str) -> Result<CabinetStage, Box<dyn std::error::Error>> {
        CabinetStage::load_from_wav(name, "./ir/1.wav", 128)
    }
}
