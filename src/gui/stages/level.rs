use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::level::LevelStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_card, SPACING_TIGHT};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LevelConfig {
    pub gain: f32,
    #[serde(default)]
    pub bypassed: bool,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self { gain: 1.0, bypassed: false }
    }
}

impl LevelConfig {
    pub const fn to_stage(&self, _sample_rate: f32) -> LevelStage {
        LevelStage::new(self.gain)
    }

    pub const fn apply(&mut self, msg: LevelMessage) -> Option<ParamUpdate> {
        match msg {
            LevelMessage::GainChanged(v) => { self.gain = v; Some(ParamUpdate::Changed("gain", v)) }
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum LevelMessage {
    GainChanged(f32),
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &LevelConfig,
    is_collapsed: bool,
    can_move_up: bool,
    can_move_down: bool,
    bypassed: bool,
) -> Element<'_, Message> {
    stage_card(tr!(stage_level), idx, is_collapsed, can_move_up, can_move_down, bypassed, || {
        column![labeled_slider(
            tr!(gain),
            0.0..=2.0,
            cfg.gain,
            move |v| Message::Stage(idx, StageMessage::Level(LevelMessage::GainChanged(v))),
            |v| format!("{v:.2}"),
            0.05
        ),]
        .spacing(SPACING_TIGHT)
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_without_bypassed_defaults_to_false() {
        let json = r#"{"gain": 1.0}"#;
        let cfg: LevelConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.bypassed);
    }

    #[test]
    fn deserialize_with_bypassed_true() {
        let json = r#"{"gain": 1.0, "bypassed": true}"#;
        let cfg: LevelConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.bypassed);
    }

    #[test]
    fn serialize_includes_bypassed() {
        let cfg = LevelConfig { gain: 1.0, bypassed: true };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"bypassed\":true"));
    }
}
