pub mod compressor;
pub mod filter;
pub mod level;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

pub use compressor::CompressorConfig;
pub use filter::FilterConfig;
pub use level::LevelConfig;
pub use poweramp::PowerAmpConfig;
pub use preamp::PreampConfig;
pub use tonestack::ToneStackConfig;

// Stage type enum
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageType {
    #[default]
    Filter,
    Preamp,
    Compressor,
    ToneStack,
    PowerAmp,
    Level,
}

impl std::fmt::Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageType::Filter => write!(f, "Filter"),
            StageType::Preamp => write!(f, "Preamp"),
            StageType::Compressor => write!(f, "Compressor"),
            StageType::ToneStack => write!(f, "Tone Stack"),
            StageType::PowerAmp => write!(f, "Power Amp"),
            StageType::Level => write!(f, "Level"),
        }
    }
}

// Stage configurations
#[derive(Debug, Clone)]
pub enum StageConfig {
    Filter(FilterConfig),
    Preamp(PreampConfig),
    Compressor(CompressorConfig),
    ToneStack(ToneStackConfig),
    PowerAmp(PowerAmpConfig),
    Level(LevelConfig),
}

impl StageConfig {
    pub fn create_default(stage_type: StageType) -> Self {
        match stage_type {
            StageType::Filter => StageConfig::Filter(FilterConfig::default()),
            StageType::Preamp => StageConfig::Preamp(PreampConfig::default()),
            StageType::Compressor => StageConfig::Compressor(CompressorConfig::default()),
            StageType::ToneStack => StageConfig::ToneStack(ToneStackConfig::default()),
            StageType::PowerAmp => StageConfig::PowerAmp(PowerAmpConfig::default()),
            StageType::Level => StageConfig::Level(LevelConfig::default()),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            StageConfig::Filter(_) => "Filter",
            StageConfig::Preamp(_) => "Preamp",
            StageConfig::Compressor(_) => "Compressor",
            StageConfig::ToneStack(_) => "Tone Stack",
            StageConfig::PowerAmp(_) => "Power Amp",
            StageConfig::Level(_) => "Level",
        }
    }
}
