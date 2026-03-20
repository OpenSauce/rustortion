use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::level::LevelConfig;
use crate::gui::components::widgets::common::{labeled_slider, stage_card, StageViewState, SPACING_TIGHT};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum LevelMessage {
    GainChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut LevelConfig, msg: LevelMessage) -> Option<ParamUpdate> {
    match msg {
        LevelMessage::GainChanged(v) => { cfg.gain = v; Some(ParamUpdate::Changed("gain", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &LevelConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(tr!(stage_level), idx, state, || {
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
    use rustortion_core::amp::stages::level::LevelConfig;

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
