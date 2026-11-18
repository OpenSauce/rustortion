//! Pure selection logic for the NAM tab: which NAM stage is focused and what the pane shows.

use crate::stages::StageConfig;

/// Chain indices of every NAM stage, in chain order.
pub fn nam_stage_indices(stages: &[StageConfig]) -> Vec<usize> {
    stages
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matches!(s, StageConfig::Nam(_)).then_some(i))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainEdit {
    Inserted(usize),
    Removed(usize),
    Swapped(usize, usize),
    Replaced,
}

/// Index arithmetic only: where a selected chain index lands after `edit`.
/// `Removed(i)` of the selected stage and `Replaced` both give `None`.
pub fn focus_after(focus: Option<usize>, edit: ChainEdit) -> Option<usize> {
    let f = focus?;
    match edit {
        ChainEdit::Inserted(at) => Some(if at <= f { f + 1 } else { f }),
        ChainEdit::Removed(at) if at == f => None,
        ChainEdit::Removed(at) => Some(if at < f { f - 1 } else { f }),
        ChainEdit::Swapped(a, b) if f == a => Some(b),
        ChainEdit::Swapped(a, b) if f == b => Some(a),
        ChainEdit::Swapped(..) => Some(f),
        ChainEdit::Replaced => None,
    }
}

/// `focus` if it names a NAM stage, otherwise the first NAM stage, otherwise `None`.
pub fn resolve_focus(focus: Option<usize>, stages: &[StageConfig]) -> Option<usize> {
    focus
        .filter(|&i| matches!(stages.get(i), Some(StageConfig::Nam(_))))
        .or_else(|| nam_stage_indices(stages).first().copied())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneState {
    /// No NAM stage in the chain.
    NoStage,
    /// A NAM stage with no model chosen.
    Empty {
        idx: usize,
    },
    Loaded {
        idx: usize,
        key: String,
    },
    /// The stage names a model the registry does not have.
    Missing {
        idx: usize,
        key: String,
    },
}

pub fn pane_state(
    stages: &[StageConfig],
    focus: Option<usize>,
    is_registered: impl Fn(&str) -> bool,
) -> PaneState {
    let Some(idx) = resolve_focus(focus, stages) else {
        return PaneState::NoStage;
    };
    let Some(StageConfig::Nam(cfg)) = stages.get(idx) else {
        return PaneState::NoStage;
    };
    match &cfg.model_name {
        None => PaneState::Empty { idx },
        Some(key) if is_registered(key) => PaneState::Loaded {
            idx,
            key: key.clone(),
        },
        Some(key) => PaneState::Missing {
            idx,
            key: key.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::{StageConfig, StageType};
    use rustortion_core::amp::stages::nam::NamConfig;

    fn nam(model: Option<&str>) -> StageConfig {
        StageConfig::Nam(NamConfig {
            model_name: model.map(str::to_owned),
            ..NamConfig::default()
        })
    }
    fn level() -> StageConfig {
        StageConfig::from(StageType::Level)
    }

    #[test]
    fn nam_stage_indices_are_in_chain_order() {
        assert_eq!(
            nam_stage_indices(&[level(), nam(None), level(), nam(Some("a"))]),
            [1, 3]
        );
        assert!(nam_stage_indices(&[level()]).is_empty());
    }

    #[test]
    fn focus_follows_its_stage_through_edits() {
        assert_eq!(focus_after(Some(3), ChainEdit::Removed(0)), Some(2));
        assert_eq!(focus_after(Some(3), ChainEdit::Removed(5)), Some(3));
        assert_eq!(focus_after(Some(3), ChainEdit::Removed(3)), None);
        assert_eq!(focus_after(Some(3), ChainEdit::Inserted(3)), Some(4));
        assert_eq!(focus_after(Some(3), ChainEdit::Inserted(4)), Some(3));
        assert_eq!(focus_after(Some(1), ChainEdit::Swapped(0, 1)), Some(0));
        assert_eq!(focus_after(Some(0), ChainEdit::Swapped(0, 1)), Some(1));
        assert_eq!(focus_after(Some(2), ChainEdit::Swapped(0, 1)), Some(2));
        assert_eq!(focus_after(Some(2), ChainEdit::Replaced), None);
        assert_eq!(focus_after(None, ChainEdit::Removed(0)), None);
    }

    #[test]
    fn a_stale_or_missing_focus_resolves_to_the_first_nam_stage() {
        let chain = [level(), nam(Some("a")), nam(Some("b"))];
        assert_eq!(resolve_focus(Some(2), &chain), Some(2));
        assert_eq!(resolve_focus(None, &chain), Some(1));
        assert_eq!(
            resolve_focus(Some(0), &chain),
            Some(1),
            "index 0 is not a NAM stage"
        );
        assert_eq!(
            resolve_focus(Some(99), &chain),
            Some(1),
            "out of range must not panic"
        );
        assert_eq!(resolve_focus(Some(0), &[level()]), None);
        assert_eq!(resolve_focus(None, &[]), None);
    }

    #[test]
    fn pane_state_covers_the_four_states() {
        let known = |key: &str| key == "a";
        assert_eq!(pane_state(&[level()], None, known), PaneState::NoStage);
        assert_eq!(
            pane_state(&[level(), nam(None)], None, known),
            PaneState::Empty { idx: 1 }
        );
        assert_eq!(
            pane_state(&[nam(Some("a"))], None, known),
            PaneState::Loaded {
                idx: 0,
                key: "a".into()
            }
        );
        assert_eq!(
            pane_state(&[nam(Some("gone"))], None, known),
            PaneState::Missing {
                idx: 0,
                key: "gone".into()
            }
        );
    }

    #[test]
    fn pane_state_follows_the_selected_stage_not_the_first() {
        let chain = [nam(Some("a")), nam(None)];
        assert_eq!(
            pane_state(&chain, Some(1), |_| true),
            PaneState::Empty { idx: 1 }
        );
    }
}
