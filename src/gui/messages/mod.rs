use crate::gui::stages::{StageConfig, StageType};

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

pub use crate::gui::stages::{
    CompressorMessage, FilterMessage, LevelMessage, MultibandSaturatorMessage, NoiseGateMessage,
    PowerAmpMessage, PreampMessage, StageMessage, ToneStackMessage,
};

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
    Settings(SettingsMessage),

    // IR Cabinet messages
    IrSelected(String),
    IrBypassed(bool),
    IrGainChanged(f32),

    // Pitch shift messages
    PitchShiftChanged(i32),

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
        Message::Preset(msg)
    }
}

impl From<HotkeyMessage> for Message {
    fn from(msg: HotkeyMessage) -> Self {
        Message::Hotkey(msg)
    }
}

impl From<MidiMessage> for Message {
    fn from(msg: MidiMessage) -> Self {
        Message::Midi(msg)
    }
}

impl From<SettingsMessage> for Message {
    fn from(msg: SettingsMessage) -> Self {
        Message::Settings(msg)
    }
}

impl From<TunerMessage> for Message {
    fn from(msg: TunerMessage) -> Self {
        Message::Tuner(msg)
    }
}
