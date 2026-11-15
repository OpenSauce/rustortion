use iced::Element;

use rustortion_core::amp::stages::nam::NamConfig;

use crate::components::nam_browser::tree::leaf_label;
use crate::components::widgets::common::{StageViewState, stage_link_row};
use crate::messages::{Message, NamTabMessage};
use crate::tr;

use super::ParamUpdate;

// --- Message ---

#[derive(Debug, Clone)]
pub enum NamMessage {
    ModelSelected(String),
    InputGainChanged(f32),
    OutputGainChanged(f32),
    MixChanged(f32),
}

pub fn apply(cfg: &mut NamConfig, msg: NamMessage) -> Option<ParamUpdate> {
    match msg {
        NamMessage::ModelSelected(key) => {
            cfg.model_name = Some(key);
            // Selecting a model is a non-float change: rebuild the stage.
            Some(ParamUpdate::NeedsStageRebuild)
        }
        NamMessage::InputGainChanged(v) => {
            cfg.input_gain_db = v;
            Some(ParamUpdate::Changed("input_gain_db", v))
        }
        NamMessage::OutputGainChanged(v) => {
            cfg.output_gain_db = v;
            Some(ParamUpdate::Changed("output_gain_db", v))
        }
        NamMessage::MixChanged(v) => {
            cfg.mix = v;
            Some(ParamUpdate::Changed("mix", v))
        }
    }
}

// --- View ---

/// "NAM 2 · leaf", or the no-model text when nothing is loaded.
fn row_label(idx: usize, cfg: &NamConfig) -> String {
    let model = cfg
        .model_name
        .as_deref()
        .map_or_else(|| tr!(nam_no_model_loaded), leaf_label);
    format!("{} {} · {model}", tr!(stage_nam), idx + 1)
}

pub fn view(idx: usize, cfg: &NamConfig, state: StageViewState) -> Element<'_, Message> {
    stage_link_row(
        row_label(idx, cfg),
        idx,
        &state,
        tr!(nam_open),
        Message::Nam(NamTabMessage::OpenStage(idx)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_amp_tab_row_names_the_stage_and_its_model() {
        let with_model = NamConfig {
            model_name: Some("Pack/Deep/leaf".into()),
            ..NamConfig::default()
        };
        assert_eq!(
            row_label(1, &with_model),
            format!("{} 2 · leaf", tr!(stage_nam))
        );
        assert_eq!(
            row_label(0, &NamConfig::default()),
            format!("{} 1 · {}", tr!(stage_nam), tr!(nam_no_model_loaded))
        );
    }

    #[test]
    fn selecting_a_model_rebuilds_the_stage() {
        let mut cfg = NamConfig::default();
        let update = apply(&mut cfg, NamMessage::ModelSelected("a".into()));
        assert_eq!(cfg.model_name.as_deref(), Some("a"));
        assert!(matches!(update, Some(ParamUpdate::NeedsStageRebuild)));
    }
}
