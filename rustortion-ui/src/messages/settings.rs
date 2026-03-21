use crate::i18n::Language;

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    Open,
    Close,
    Apply,
    RefreshPorts,
    InputPortChanged(String),
    OutputLeftPortChanged(String),
    OutputRightPortChanged(String),
    BufferSizeChanged(u32),
    SampleRateChanged(u32),
    OversamplingFactorChanged(u32),
    LanguageChanged(Language),
}
