#[derive(Debug, Clone)]
pub enum PresetMessage {
    Select(String),
    Save(String),
    Update,
    Delete(String),
    Gui(PresetGuiMessage),
}

#[derive(Debug, Clone)]
pub enum PresetGuiMessage {
    CancelSave,
    ShowSave,
    NameChanged(String),
    ConfirmOverwrite,
    CancelOverwrite,
}
