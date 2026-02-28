#[derive(Debug, Clone)]
pub enum HotkeyMessage {
    Open,
    Close,
    StartLearning,
    CancelLearning,
    PresetSelected(String),
    ConfirmMapping,
    RemoveMapping(usize),
}
