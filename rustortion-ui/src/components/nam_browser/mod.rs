use std::collections::HashSet;

use crate::stages::StageConfig;
use selection::ChainEdit;

pub mod selection;
pub mod tree;
mod view;

pub use view::NamViewContext;

#[derive(Debug, Default)]
pub struct NamBrowser {
    filter: String,
    open_folders: HashSet<String>,
    focus: Option<usize>,
}

impl NamBrowser {
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// Opens a closed folder, closes an open one.
    pub fn toggle_folder(&mut self, path: String) {
        if !self.open_folders.remove(&path) {
            self.open_folders.insert(path);
        }
    }

    pub const fn focus_stage(&mut self, idx: usize) {
        self.focus = Some(idx);
    }

    /// Call after every edit to `SharedApp::stages`.
    pub fn chain_edited(&mut self, edit: ChainEdit) {
        self.focus = selection::focus_after(self.focus, edit);
    }

    /// The selected NAM stage's chain index, resolved against the live chain.
    pub fn focused_stage(&self, stages: &[StageConfig]) -> Option<usize> {
        selection::resolve_focus(self.focus, stages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::{StageConfig, StageType};
    use rustortion_core::amp::stages::nam::NamConfig;

    #[test]
    fn browser_keeps_its_selection_on_the_same_stage() {
        use selection::ChainEdit;
        let nam = |m: &str| {
            StageConfig::Nam(NamConfig {
                model_name: Some(m.to_owned()),
                ..NamConfig::default()
            })
        };
        let level = || StageConfig::from(StageType::Level);

        let mut browser = NamBrowser::default();
        let mut chain = vec![level(), nam("a"), nam("b")];
        assert_eq!(
            browser.focused_stage(&chain),
            Some(1),
            "defaults to the first NAM stage"
        );

        browser.focus_stage(2);
        chain.remove(0);
        browser.chain_edited(ChainEdit::Removed(0));
        assert_eq!(
            browser.focused_stage(&chain),
            Some(1),
            "still the stage holding b"
        );

        chain.remove(1);
        browser.chain_edited(ChainEdit::Removed(1));
        assert_eq!(
            browser.focused_stage(&chain),
            Some(0),
            "falls to the first remaining NAM stage"
        );

        browser.toggle_folder("Pack".into());
        browser.toggle_folder("Pack".into());
        assert!(
            browser.open_folders.is_empty(),
            "toggling twice closes it again"
        );
    }
}
