use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::amp::stages::Stage;
use crate::amp::stages::compressor::CompressorConfig;
use crate::amp::stages::delay::DelayConfig;
use crate::amp::stages::eq::EqConfig;
use crate::amp::stages::level::LevelConfig;
use crate::amp::stages::multiband_saturator::MultibandSaturatorConfig;
use crate::amp::stages::noise_gate::NoiseGateConfig;
use crate::amp::stages::poweramp::PowerAmpConfig;
use crate::amp::stages::preamp::PreampConfig;
use crate::amp::stages::reverb::ReverbConfig;
use crate::amp::stages::tonestack::ToneStackConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageCategory {
    Amp,
    Effect,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageType {
    #[default]
    Preamp,
    Compressor,
    ToneStack,
    PowerAmp,
    Level,
    NoiseGate,
    MultibandSaturator,
    Delay,
    Reverb,
    Eq,
}

impl StageType {
    pub const ALL: &[Self] = &[
        Self::Preamp,
        Self::Compressor,
        Self::ToneStack,
        Self::PowerAmp,
        Self::Level,
        Self::NoiseGate,
        Self::MultibandSaturator,
        Self::Delay,
        Self::Reverb,
        Self::Eq,
    ];

    pub const fn category(self) -> StageCategory {
        match self {
            Self::Preamp
            | Self::Compressor
            | Self::ToneStack
            | Self::PowerAmp
            | Self::Level
            | Self::NoiseGate
            | Self::MultibandSaturator => StageCategory::Amp,
            Self::Delay | Self::Reverb | Self::Eq => StageCategory::Effect,
        }
    }

    pub fn for_category(cat: StageCategory) -> Vec<Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(|s| s.category() == cat)
            .collect()
    }
}

impl Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preamp => write!(f, "Preamp"),
            Self::Compressor => write!(f, "Compressor"),
            Self::ToneStack => write!(f, "Tone Stack"),
            Self::PowerAmp => write!(f, "Power Amp"),
            Self::Level => write!(f, "Level"),
            Self::NoiseGate => write!(f, "Noise Gate"),
            Self::MultibandSaturator => write!(f, "Multiband Saturator"),
            Self::Delay => write!(f, "Delay"),
            Self::Reverb => write!(f, "Reverb"),
            Self::Eq => write!(f, "EQ"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageConfig {
    Preamp(PreampConfig),
    Compressor(CompressorConfig),
    ToneStack(ToneStackConfig),
    PowerAmp(PowerAmpConfig),
    Level(LevelConfig),
    NoiseGate(NoiseGateConfig),
    MultibandSaturator(MultibandSaturatorConfig),
    Delay(DelayConfig),
    Reverb(ReverbConfig),
    Eq(EqConfig),
}

impl From<StageType> for StageConfig {
    fn from(kind: StageType) -> Self {
        match kind {
            StageType::Preamp => Self::Preamp(PreampConfig::default()),
            StageType::Compressor => Self::Compressor(CompressorConfig::default()),
            StageType::ToneStack => Self::ToneStack(ToneStackConfig::default()),
            StageType::PowerAmp => Self::PowerAmp(PowerAmpConfig::default()),
            StageType::Level => Self::Level(LevelConfig::default()),
            StageType::NoiseGate => Self::NoiseGate(NoiseGateConfig::default()),
            StageType::MultibandSaturator => {
                Self::MultibandSaturator(MultibandSaturatorConfig::default())
            }
            StageType::Delay => Self::Delay(DelayConfig::default()),
            StageType::Reverb => Self::Reverb(ReverbConfig::default()),
            StageType::Eq => Self::Eq(EqConfig::default()),
        }
    }
}

impl StageConfig {
    pub fn to_runtime(&self, sample_rate: f32) -> Box<dyn Stage> {
        match self {
            Self::Preamp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::Compressor(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::ToneStack(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::PowerAmp(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::Level(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::NoiseGate(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::MultibandSaturator(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::Delay(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::Reverb(cfg) => Box::new(cfg.to_stage(sample_rate)),
            Self::Eq(cfg) => Box::new(cfg.to_stage(sample_rate)),
        }
    }

    pub const fn stage_type(&self) -> StageType {
        match self {
            Self::Preamp(_) => StageType::Preamp,
            Self::Compressor(_) => StageType::Compressor,
            Self::ToneStack(_) => StageType::ToneStack,
            Self::PowerAmp(_) => StageType::PowerAmp,
            Self::Level(_) => StageType::Level,
            Self::NoiseGate(_) => StageType::NoiseGate,
            Self::MultibandSaturator(_) => StageType::MultibandSaturator,
            Self::Delay(_) => StageType::Delay,
            Self::Reverb(_) => StageType::Reverb,
            Self::Eq(_) => StageType::Eq,
        }
    }

    pub const fn category(&self) -> StageCategory {
        self.stage_type().category()
    }

    pub const fn bypassed(&self) -> bool {
        match self {
            Self::Preamp(cfg) => cfg.bypassed,
            Self::Compressor(cfg) => cfg.bypassed,
            Self::ToneStack(cfg) => cfg.bypassed,
            Self::PowerAmp(cfg) => cfg.bypassed,
            Self::Level(cfg) => cfg.bypassed,
            Self::NoiseGate(cfg) => cfg.bypassed,
            Self::MultibandSaturator(cfg) => cfg.bypassed,
            Self::Delay(cfg) => cfg.bypassed,
            Self::Reverb(cfg) => cfg.bypassed,
            Self::Eq(cfg) => cfg.bypassed,
        }
    }

    pub const fn set_bypassed(&mut self, bypassed: bool) {
        match self {
            Self::Preamp(cfg) => cfg.bypassed = bypassed,
            Self::Compressor(cfg) => cfg.bypassed = bypassed,
            Self::ToneStack(cfg) => cfg.bypassed = bypassed,
            Self::PowerAmp(cfg) => cfg.bypassed = bypassed,
            Self::Level(cfg) => cfg.bypassed = bypassed,
            Self::NoiseGate(cfg) => cfg.bypassed = bypassed,
            Self::MultibandSaturator(cfg) => cfg.bypassed = bypassed,
            Self::Delay(cfg) => cfg.bypassed = bypassed,
            Self::Reverb(cfg) => cfg.bypassed = bypassed,
            Self::Eq(cfg) => cfg.bypassed = bypassed,
        }
    }
}
