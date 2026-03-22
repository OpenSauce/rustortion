use crate::stages::{StageConfig, StageType};
use crate::tabs::Tab;
use rustortion_core::preset::InputFilterConfig;

pub mod hotkey;
pub mod midi;
pub mod preset;
pub mod settings;
pub mod tuner;

pub use hotkey::*;
pub use midi::*;
pub use preset::*;
pub use settings::*;
pub use tuner::*;

pub use crate::stages::{
    CompressorMessage, LevelMessage, MultibandSaturatorMessage, NoiseGateMessage, PowerAmpMessage,
    PreampMessage, StageMessage, ToneStackMessage,
};

#[derive(Debug, Clone)]
pub enum Message {
    // Tab navigation
    TabSelected(Tab),

    // App-level messages
    AddStage,
    RemoveStage(usize),
    MoveStageUp(usize),
    MoveStageDown(usize),
    ToggleStageCollapse(usize),
    ToggleAllStagesCollapse,
    ToggleStageBypass(usize),
    StageTypeSelected(StageType),
    RebuildTick,
    SetStages(Vec<StageConfig>),

    // Input filter messages
    InputFilterHighpassToggle(bool),
    InputFilterHighpassCutoff(f32),
    InputFilterLowpassToggle(bool),
    InputFilterLowpassCutoff(f32),
    SetInputFilters(InputFilterConfig),

    // Preset settings
    Preset(PresetMessage),

    // Recording messages
    StartRecording,
    StopRecording,

    // Settings messages
    Settings(SettingsMessage),

    // IR Cabinet messages
    IrSelected(String),
    IrBypassed(bool),
    IrGainChanged(f32),

    // Pitch shift messages
    PitchShiftChanged(i32),

    // Oversampling messages
    OversamplingChanged(u32),

    // Stage-specific messages
    Stage(usize, StageMessage),

    // Tuner messages
    Tuner(TunerMessage),

    // MIDI messages
    Midi(MidiMessage),

    // Hotkey messages
    Hotkey(HotkeyMessage),
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Peak meter messages
    PeakMeterUpdate,
}

impl From<PresetMessage> for Message {
    fn from(msg: PresetMessage) -> Self {
        Self::Preset(msg)
    }
}

impl From<HotkeyMessage> for Message {
    fn from(msg: HotkeyMessage) -> Self {
        Self::Hotkey(msg)
    }
}

impl From<MidiMessage> for Message {
    fn from(msg: MidiMessage) -> Self {
        Self::Midi(msg)
    }
}

impl From<SettingsMessage> for Message {
    fn from(msg: SettingsMessage) -> Self {
        Self::Settings(msg)
    }
}

impl From<TunerMessage> for Message {
    fn from(msg: TunerMessage) -> Self {
        Self::Tuner(msg)
    }
}
