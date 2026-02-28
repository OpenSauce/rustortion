use crate::gui::messages::Message;
use crate::tr;
use iced::Element;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

macro_rules! stage_registry {
    (
        default = $Default:ident;
        $( $Variant:ident => $module:ident, $Config:ident, $Msg:ident, $tr_key:ident );+ $(;)?
    ) => {
        // --- Module declarations ---
        $( pub mod $module; )+

        // --- Re-exports ---
        $( pub use $module::{$Config, $Msg}; )+

        // --- StageType ---
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum StageType {
            $( $Variant, )+
        }

        impl Default for StageType {
            fn default() -> Self {
                Self::$Default
            }
        }

        impl Display for StageType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $( StageType::$Variant => write!(f, "{}", tr!($tr_key)), )+
                }
            }
        }

        // --- StageMessage ---
        #[derive(Debug, Clone)]
        pub enum StageMessage {
            $( $Variant($Msg), )+
        }

        // --- StageConfig ---
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum StageConfig {
            $( $Variant($Config), )+
        }

        impl From<StageType> for StageConfig {
            fn from(kind: StageType) -> Self {
                match kind {
                    $( StageType::$Variant => StageConfig::$Variant($Config::default()), )+
                }
            }
        }

        impl StageConfig {
            pub fn to_runtime(&self, sample_rate: f32) -> Box<dyn crate::amp::stages::Stage> {
                match self {
                    $( StageConfig::$Variant(cfg) => Box::new(cfg.to_stage(sample_rate)), )+
                }
            }

            pub const fn apply(&mut self, msg: StageMessage) -> bool {
                match (self, msg) {
                    $(
                        (StageConfig::$Variant(cfg), StageMessage::$Variant(m)) => {
                            cfg.apply(m);
                            true
                        }
                    )+
                    _ => false,
                }
            }

            pub fn view(&self, idx: usize, total_stages: usize, is_collapsed: bool) -> Element<'_, Message> {
                match self {
                    $( StageConfig::$Variant(cfg) => $module::view(idx, cfg, total_stages, is_collapsed), )+
                }
            }
        }
    };
}

stage_registry! {
    default = Filter;
    Filter             => filter,               FilterConfig,             FilterMessage,             stage_filter;
    Preamp             => preamp,               PreampConfig,             PreampMessage,             stage_preamp;
    Compressor         => compressor,           CompressorConfig,         CompressorMessage,         stage_compressor;
    ToneStack          => tonestack,            ToneStackConfig,          ToneStackMessage,          stage_tone_stack;
    PowerAmp           => poweramp,             PowerAmpConfig,           PowerAmpMessage,           stage_power_amp;
    Level              => level,                LevelConfig,              LevelMessage,              stage_level;
    NoiseGate          => noise_gate,           NoiseGateConfig,          NoiseGateMessage,          stage_noise_gate;
    MultibandSaturator => multiband_saturator,  MultibandSaturatorConfig, MultibandSaturatorMessage, stage_multiband_saturator;
}
