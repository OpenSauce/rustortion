pub use rustortion_core::preset::stage_config::{StageCategory, StageConfig, StageType};

use crate::messages::Message;
use iced::Element;

/// Result of applying a stage message to a config.
#[derive(Debug, Clone)]
pub enum ParamUpdate {
    /// A float parameter changed -- forward via `SetParameter` message.
    Changed(&'static str, f32),
    /// A non-float parameter changed -- rebuild this stage only.
    NeedsStageRebuild,
}

macro_rules! gui_stage_registry {
    (
        $( $Variant:ident => $module:ident, $Msg:ident, $tr_key:ident );+ $(;)?
    ) => {
        // --- Module declarations ---
        $( pub mod $module; )+

        // --- Re-exports ---
        $( pub use $module::$Msg; )+

        // --- StageMessage ---
        #[derive(Debug, Clone)]
        pub enum StageMessage {
            $( $Variant($Msg), )+
        }

        pub fn stage_type_label(st: &StageType) -> String {
            match st {
                $( StageType::$Variant => crate::tr!($tr_key).to_string(), )+
            }
        }

        pub const fn apply_stage_config(cfg: &mut StageConfig, msg: StageMessage) -> Option<ParamUpdate> {
            match (cfg, msg) {
                $(
                    (StageConfig::$Variant(c), StageMessage::$Variant(m)) => {
                        $module::apply(c, m)
                    }
                )+
                _ => None,
            }
        }

        pub fn view_stage_config(cfg: &StageConfig, idx: usize, state: crate::components::widgets::common::StageViewState) -> Element<'_, Message> {
            match cfg {
                $( StageConfig::$Variant(c) => $module::view(idx, c, state), )+
            }
        }
    };
}

gui_stage_registry! {
    Preamp             => preamp,               PreampMessage,             stage_preamp;
    Compressor         => compressor,           CompressorMessage,         stage_compressor;
    ToneStack          => tonestack,            ToneStackMessage,          stage_tone_stack;
    PowerAmp           => poweramp,             PowerAmpMessage,           stage_power_amp;
    Level              => level,                LevelMessage,              stage_level;
    NoiseGate          => noise_gate,           NoiseGateMessage,          stage_noise_gate;
    MultibandSaturator => multiband_saturator,  MultibandSaturatorMessage, stage_multiband_saturator;
    Delay              => delay,                DelayMessage,              stage_delay;
    Reverb             => reverb,               ReverbMessage,             stage_reverb;
    Eq                 => eq,                   EqMessage,                 stage_eq;
}
