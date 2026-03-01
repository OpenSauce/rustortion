use crate::gui::messages::Message;
use crate::tr;
use iced::Element;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageCategory {
    Amp,
    Effect,
}

macro_rules! stage_registry {
    (
        default = $Default:ident;
        $( $Variant:ident => $module:ident, $Config:ident, $Msg:ident, $tr_key:ident, $category:ident );+ $(;)?
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

        impl StageType {
            pub const ALL: &[Self] = &[
                $( Self::$Variant, )+
            ];

            pub const fn category(self) -> StageCategory {
                match self {
                    $( Self::$Variant => StageCategory::$category, )+
                }
            }

            pub fn for_category(cat: StageCategory) -> Vec<Self> {
                Self::ALL.iter().copied().filter(|s| s.category() == cat).collect()
            }
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

            pub fn view(&self, idx: usize, is_collapsed: bool, can_move_up: bool, can_move_down: bool) -> Element<'_, Message> {
                match self {
                    $( StageConfig::$Variant(cfg) => $module::view(idx, cfg, is_collapsed, can_move_up, can_move_down), )+
                }
            }

            pub const fn stage_type(&self) -> StageType {
                match self {
                    $( StageConfig::$Variant(_) => StageType::$Variant, )+
                }
            }

            pub const fn category(&self) -> StageCategory {
                self.stage_type().category()
            }
        }
    };
}

stage_registry! {
    default = Preamp;
    Preamp             => preamp,               PreampConfig,             PreampMessage,             stage_preamp,             Amp;
    Compressor         => compressor,           CompressorConfig,         CompressorMessage,         stage_compressor,         Amp;
    ToneStack          => tonestack,            ToneStackConfig,          ToneStackMessage,          stage_tone_stack,         Amp;
    PowerAmp           => poweramp,             PowerAmpConfig,           PowerAmpMessage,           stage_power_amp,          Amp;
    Level              => level,                LevelConfig,              LevelMessage,              stage_level,              Amp;
    NoiseGate          => noise_gate,           NoiseGateConfig,          NoiseGateMessage,          stage_noise_gate,         Amp;
    MultibandSaturator => multiband_saturator,  MultibandSaturatorConfig, MultibandSaturatorMessage, stage_multiband_saturator, Amp;
    Delay              => delay,                DelayConfig,              DelayMessage,              stage_delay,              Effect;
    Reverb             => reverb,               ReverbConfig,             ReverbMessage,             stage_reverb,             Effect;
}
