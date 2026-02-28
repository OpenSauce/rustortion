#[derive(Debug, Clone)]
pub enum MidiMessage {
    Open,
    Close,
    ControllerSelected(String),
    Disconnect,
    RefreshControllers,
    StartLearning,
    CancelLearning,
    PresetForMappingSelected(String),
    ConfirmMapping,
    RemoveMapping(usize),
    Update,
}
