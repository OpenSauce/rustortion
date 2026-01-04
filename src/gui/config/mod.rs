pub mod compressor;
pub mod filter;
pub mod level;
pub mod noise_gate;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

pub use compressor::CompressorConfig;
pub use filter::FilterConfig;
pub use level::LevelConfig;
pub use noise_gate::NoiseGateConfig;
pub use poweramp::PowerAmpConfig;
pub use preamp::PreampConfig;
pub use tonestack::ToneStackConfig;

use crate::gui::messages::{
    CompressorMessage, FilterMessage, LevelMessage, NoiseGateMessage, PowerAmpMessage,
    PreampMessage, StageMessage, ToneStackMessage,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

// Stage type enum
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
}

impl Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageType::Filter => write!(f, "Filter"),
            StageType::Preamp => write!(f, "Preamp"),
            StageType::Compressor => write!(f, "Compressor"),
            StageType::ToneStack => write!(f, "Tone Stack"),
            StageType::PowerAmp => write!(f, "Power Amp"),
            StageType::Level => write!(f, "Level"),
            StageType::NoiseGate => write!(f, "Noise Gate"),
        }
    }
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
        }
    }
}

// Stage configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageConfig {
    Filter(FilterConfig),
    Preamp(PreampConfig),
    Compressor(CompressorConfig),
    ToneStack(ToneStackConfig),
    PowerAmp(PowerAmpConfig),
    Level(LevelConfig),
    NoiseGate(NoiseGateConfig),
}

impl StageConfig {
    pub fn to_runtime(&self, sample_rate: f32) -> Box<dyn crate::sim::stages::Stage> {
        match self {
            StageConfig::Filter(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Preamp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Compressor(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::ToneStack(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::PowerAmp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            StageConfig::Level(cfg) => Box::new(cfg.to_stage()),
            StageConfig::NoiseGate(cfg) => Box::new(cfg.to_stage(sample_rate)),
        }
    }

    pub fn apply(&mut self, msg: StageMessage) -> bool {
        match (self, msg) {
            (StageConfig::Filter(cfg), StageMessage::Filter(m)) => {
                match m {
                    FilterMessage::TypeChanged(t) => cfg.filter_type = t,
                    FilterMessage::CutoffChanged(v) => cfg.cutoff_hz = v,
                }
                true
            }
            (StageConfig::Preamp(cfg), StageMessage::Preamp(m)) => {
                match m {
                    PreampMessage::GainChanged(v) => cfg.gain = v,
                    PreampMessage::BiasChanged(v) => cfg.bias = v,
                    PreampMessage::ClipperChanged(c) => cfg.clipper_type = c,
                }
                true
            }
            (StageConfig::Compressor(cfg), StageMessage::Compressor(m)) => {
                match m {
                    CompressorMessage::ThresholdChanged(v) => cfg.threshold_db = v,
                    CompressorMessage::RatioChanged(v) => cfg.ratio = v,
                    CompressorMessage::AttackChanged(v) => cfg.attack_ms = v,
                    CompressorMessage::ReleaseChanged(v) => cfg.release_ms = v,
                    CompressorMessage::MakeupChanged(v) => cfg.makeup_db = v,
                }
                true
            }
            (StageConfig::ToneStack(cfg), StageMessage::ToneStack(m)) => {
                match m {
                    ToneStackMessage::ModelChanged(mo) => cfg.model = mo,
                    ToneStackMessage::BassChanged(v) => cfg.bass = v,
                    ToneStackMessage::MidChanged(v) => cfg.mid = v,
                    ToneStackMessage::TrebleChanged(v) => cfg.treble = v,
                    ToneStackMessage::PresenceChanged(v) => cfg.presence = v,
                }
                true
            }
            (StageConfig::PowerAmp(cfg), StageMessage::PowerAmp(m)) => {
                match m {
                    PowerAmpMessage::TypeChanged(t) => cfg.amp_type = t,
                    PowerAmpMessage::DriveChanged(v) => cfg.drive = v,
                    PowerAmpMessage::SagChanged(v) => cfg.sag = v,
                }
                true
            }
            (StageConfig::Level(cfg), StageMessage::Level(m)) => {
                match m {
                    LevelMessage::GainChanged(v) => cfg.gain = v,
                }
                true
            }
            (StageConfig::NoiseGate(cfg), StageMessage::NoiseGate(m)) => {
                match m {
                    NoiseGateMessage::ThresholdChanged(v) => cfg.threshold_db = v,
                    NoiseGateMessage::RatioChanged(v) => cfg.ratio = v,
                    NoiseGateMessage::AttackChanged(v) => cfg.attack_ms = v,
                    NoiseGateMessage::HoldChanged(v) => cfg.hold_ms = v,
                    NoiseGateMessage::ReleaseChanged(v) => cfg.release_ms = v,
                }
                true
            }
            _ => false,
        }
    }
}
