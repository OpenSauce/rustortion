use crate::stages::StageCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Amp,
    Effects,
    Cabinet,
    Io,
}

impl Tab {
    pub const fn stage_category(self) -> Option<StageCategory> {
        match self {
            Self::Amp => Some(StageCategory::Amp),
            Self::Effects => Some(StageCategory::Effect),
            _ => None,
        }
    }
}
