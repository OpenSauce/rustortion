/// Messages from the NAM tab.
#[derive(Debug, Clone)]
pub enum NamTabMessage {
    FilterChanged(String),
    /// Folder path, as in `ListRow::Folder::path`.
    FolderToggled(String),
    /// Registry key of the model that was clicked.
    ModelClicked(String),
    /// Chain index picked in the switcher.
    StageFocused(usize),
    /// Select this chain index and switch to the NAM tab.
    OpenStage(usize),
    Rescan,
    OpenFolder,
}
