#[derive(Debug, Clone)]
pub enum PresetMessage {
    Select(String),
    /// Save under a new name. Prompts first if a preset by that name exists.
    Save(String),
    /// Save under a name that has already passed the overwrite confirmation.
    /// Kept separate from `Save` so confirming cannot re-trigger the prompt.
    SaveConfirmed(String),
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
