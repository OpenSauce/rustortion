pub mod compressor;
pub mod filter;
pub mod level;
pub mod multiband_saturator;
pub mod noise_gate;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

pub use compressor::{CompressorConfig, CompressorMessage};
pub use filter::{FilterConfig, FilterMessage};
pub use level::{LevelConfig, LevelMessage};
pub use multiband_saturator::{MultibandSaturatorConfig, MultibandSaturatorMessage};
pub use noise_gate::{NoiseGateConfig, NoiseGateMessage};
pub use poweramp::{PowerAmpConfig, PowerAmpMessage};
pub use preamp::{PreampConfig, PreampMessage};
pub use tonestack::{ToneStackConfig, ToneStackMessage};

use crate::gui::messages::Message;
use crate::tr;
use iced::Element;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

// --- StageType ---

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageType {
    #[default]
    Filter,
    Preamp,
    Compressor,
    ToneStack,
    PowerAmp,
    Level,
    NoiseGate,
    MultibandSaturator,
}

impl Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageType::Filter => write!(f, "{}", tr!(stage_filter)),
            StageType::Preamp => write!(f, "{}", tr!(stage_preamp)),
            StageType::Compressor => write!(f, "{}", tr!(stage_compressor)),
            StageType::ToneStack => write!(f, "{}", tr!(stage_tone_stack)),
            StageType::PowerAmp => write!(f, "{}", tr!(stage_power_amp)),
            StageType::Level => write!(f, "{}", tr!(stage_level)),
            StageType::NoiseGate => write!(f, "{}", tr!(stage_noise_gate)),
            StageType::MultibandSaturator => write!(f, "{}", tr!(stage_multiband_saturator)),
        }
    }
}

// --- StageMessage ---

#[derive(Debug, Clone)]
pub enum StageMessage {
    Filter(FilterMessage),
    Preamp(PreampMessage),
    Compressor(CompressorMessage),
    ToneStack(ToneStackMessage),
    PowerAmp(PowerAmpMessage),
    Level(LevelMessage),
    NoiseGate(NoiseGateMessage),
    MultibandSaturator(MultibandSaturatorMessage),
}

// --- StageConfig ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageConfig {
    Filter(FilterConfig),
    Preamp(PreampConfig),
    Compressor(CompressorConfig),
    ToneStack(ToneStackConfig),
    PowerAmp(PowerAmpConfig),
    Level(LevelConfig),
    NoiseGate(NoiseGateConfig),
    MultibandSaturator(MultibandSaturatorConfig),
}

impl From<StageType> for StageConfig {
    fn from(kind: StageType) -> Self {
        match kind {
            StageType::Filter => StageConfig::Filter(FilterConfig::default()),
            StageType::Preamp => StageConfig::Preamp(PreampConfig::default()),
            StageType::Compressor => StageConfig::Compressor(CompressorConfig::default()),
            StageType::ToneStack => StageConfig::ToneStack(ToneStackConfig::default()),
            StageType::PowerAmp => StageConfig::PowerAmp(PowerAmpConfig::default()),
            StageType::Level => StageConfig::Level(LevelConfig::default()),
            StageType::NoiseGate => StageConfig::NoiseGate(NoiseGateConfig::default()),
            StageType::MultibandSaturator => {
                StageConfig::MultibandSaturator(MultibandSaturatorConfig::default())
            }
        }
    }
}

impl StageConfig {
    pub fn to_runtime(&self, sample_rate: f32) -> Box<dyn crate::amp::stages::Stage> {
        match self {
            StageConfig::Filter(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Preamp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Compressor(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::ToneStack(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::PowerAmp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Level(cfg) => Box::new(cfg.to_stage()),
            StageConfig::NoiseGate(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::MultibandSaturator(cfg) => Box::new(cfg.to_stage(sample_rate)),
        }
    }

    pub fn apply(&mut self, msg: StageMessage) -> bool {
        match (self, msg) {
            (StageConfig::Filter(cfg), StageMessage::Filter(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::Preamp(cfg), StageMessage::Preamp(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::Compressor(cfg), StageMessage::Compressor(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::ToneStack(cfg), StageMessage::ToneStack(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::PowerAmp(cfg), StageMessage::PowerAmp(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::Level(cfg), StageMessage::Level(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::NoiseGate(cfg), StageMessage::NoiseGate(m)) => {
                cfg.apply(m);
                true
            }
            (StageConfig::MultibandSaturator(cfg), StageMessage::MultibandSaturator(m)) => {
                cfg.apply(m);
                true
            }
            _ => false,
        }
    }

    pub fn view(&self, idx: usize, total_stages: usize) -> Element<'_, Message> {
        match self {
            StageConfig::Filter(cfg) => filter::view(idx, cfg, total_stages),
            StageConfig::Preamp(cfg) => preamp::view(idx, cfg, total_stages),
            StageConfig::Compressor(cfg) => compressor::view(idx, cfg, total_stages),
            StageConfig::ToneStack(cfg) => tonestack::view(idx, cfg, total_stages),
            StageConfig::PowerAmp(cfg) => poweramp::view(idx, cfg, total_stages),
            StageConfig::Level(cfg) => level::view(idx, cfg, total_stages),
            StageConfig::NoiseGate(cfg) => noise_gate::view(idx, cfg, total_stages),
            StageConfig::MultibandSaturator(cfg) => {
                multiband_saturator::view(idx, cfg, total_stages)
            }
        }
    }
}
