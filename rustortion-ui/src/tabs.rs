use serde::{Deserialize, Serialize};

use crate::stages::{StageCategory, StageConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Amp,
    Nam,
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

/// The tab a stage is edited on: NAM stages on `Tab::Nam`, the rest by category.
pub const fn tab_for_stage(cfg: &StageConfig) -> Tab {
    match cfg {
        StageConfig::Nam(_) => Tab::Nam,
        _ => match cfg.category() {
            StageCategory::Amp => Tab::Amp,
            StageCategory::Effect => Tab::Effects,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::StageType;

    #[test]
    fn nam_stages_belong_to_the_nam_tab_and_the_rest_to_their_category() {
        assert_eq!(tab_for_stage(&StageConfig::from(StageType::Nam)), Tab::Nam);
        assert_eq!(
            tab_for_stage(&StageConfig::from(StageType::Preamp)),
            Tab::Amp
        );
        assert_eq!(
            tab_for_stage(&StageConfig::from(StageType::Reverb)),
            Tab::Effects
        );
        assert_eq!(Tab::Nam.stage_category(), None);
        assert_eq!(Tab::default(), Tab::Amp);
    }
}
