use crate::gui::config::{StageConfig, StageType};
use crate::i18n::Language;

pub mod hotkey;
pub mod preset;
pub mod stage;

pub use hotkey::*;
pub use preset::*;
pub use stage::*;

#[derive(Debug, Clone)]
pub enum Message {
    // App-level messages
    AddStage,
    RemoveStage(usize),
    MoveStageUp(usize),
    MoveStageDown(usize),
    StageTypeSelected(StageType),
    RebuildTick,
    SetStages(Vec<StageConfig>),

    // Preset settings
    Preset(PresetMessage),

    // Recording messages
    StartRecording,
    StopRecording,

    // Settings messages
    OpenSettings,
    CancelSettings,
    ApplySettings,
    RefreshPorts,
    InputPortChanged(String),
    OutputLeftPortChanged(String),
    OutputRightPortChanged(String),
    BufferSizeChanged(u32),
    SampleRateChanged(u32),
    OversamplingFactorChanged(u32),
    LanguageChanged(Language),

    // IR Cabinet messages
    IrSelected(String),
    IrBypassed(bool),
    IrGainChanged(f32),

    // Pitch shift messages
    PitchShiftChanged(i32),

    // Stage-specific messages
    Stage(usize, StageMessage),

    // Tuner messages
    ToggleTuner,
    TunerUpdate,

    OpenMidi,
    MidiClose,
    MidiControllerSelected(String),
    MidiDisconnect,
    MidiRefreshControllers,
    MidiStartLearning,
    MidiCancelLearning,
    MidiPresetForMappingSelected(String),
    MidiConfirmMapping,
    MidiRemoveMapping(usize),
    MidiUpdate,

    // Hotkey messages
    Hotkey(HotkeyMessage),
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Peak meter messages
    PeakMeterUpdate,
}

impl From<PresetMessage> for Message {
    fn from(msg: PresetMessage) -> Self {
        Message::Preset(msg)
    }
}

impl From<HotkeyMessage> for Message {
    fn from(msg: HotkeyMessage) -> Self {
        Message::Hotkey(msg)
    }
}
