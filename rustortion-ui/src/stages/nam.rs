use iced::widget::{column, pick_list, row, text};
use iced::{Alignment, Element, Length};

use rustortion_core::amp::stages::nam::NamConfig;
use rustortion_core::nam::registry;

use crate::components::widgets::common::{
    labeled_slider, stage_card, StageViewState, SPACING_NORMAL, SPACING_TIGHT,
};
use crate::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum NamMessage {
    ModelSelected(Option<String>),
    InputGainChanged(f32),
    OutputGainChanged(f32),
    MixChanged(f32),
}

/// A pick-list entry: either a named model or an explicit "no model" choice that
/// clears the selection back to passthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NamModelChoice {
    None,
    Model(String),
}

impl NamModelChoice {
    fn into_option(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Model(name) => Some(name),
        }
    }
}

impl std::fmt::Display for NamModelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str(tr!(nam_no_model)),
            Self::Model(name) => f.write_str(name),
        }
    }
}

// --- Apply ---

pub fn apply(cfg: &mut NamConfig, msg: NamMessage) -> Option<ParamUpdate> {
    match msg {
        NamMessage::ModelSelected(name) => {
            cfg.model_name = name;
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

pub fn view(idx: usize, cfg: &NamConfig, state: StageViewState) -> Element<'_, Message> {
    let model_name = cfg.model_name.clone();
    let input_gain_db = cfg.input_gain_db;
    let output_gain_db = cfg.output_gain_db;
    let mix = cfg.mix;

    stage_card(tr!(stage_nam), idx, state, move || {
        // "(None)" first so a selected model can be cleared back to passthrough.
        let mut choices = vec![NamModelChoice::None];
        choices.extend(registry::available_names().into_iter().map(NamModelChoice::Model));
        let selected = model_name
            .clone()
            .map_or(NamModelChoice::None, NamModelChoice::Model);

        let model_selector = row![
            text(tr!(nam_model)).width(Length::FillPortion(3)),
            pick_list(choices, Some(selected), move |choice| {
                Message::Stage(
                    idx,
                    StageMessage::Nam(NamMessage::ModelSelected(choice.into_option())),
                )
            })
            .placeholder(tr!(nam_no_model))
            .width(Length::FillPortion(7)),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);

        // Read-only info: the selected model's native sample rate (or "not found").
        // When the native rate mismatches the engine rate the model is bypassed (dry
        // passthrough, no resampling) — surface that here using both rates.
        let engine_rate = state.engine_sample_rate;
        let info_line: Element<'_, Message> = match model_name.as_deref() {
            Some(name) => match registry::get(name) {
                Some(model) => {
                    let native_rate = model.sample_rate() as u32;
                    if native_rate.abs_diff(engine_rate) > 1 {
                        text(format!(
                            "{}: {native_rate} Hz · {engine_rate} Hz",
                            tr!(nam_rate_mismatch_bypassed)
                        ))
                    } else {
                        text(format!("{}: {native_rate} Hz", tr!(nam_native_rate)))
                    }
                }
                None => text(tr!(nam_model_not_found)),
            }
            .into(),
            None => text(String::new()).into(),
        };

        column![
            model_selector,
            info_line,
            labeled_slider(
                tr!(nam_input_gain),
                -24.0..=24.0,
                input_gain_db,
                move |v| Message::Stage(idx, StageMessage::Nam(NamMessage::InputGainChanged(v))),
                |v| format!("{v:+.1} dB"),
                0.1,
            ),
            labeled_slider(
                tr!(nam_output_gain),
                -24.0..=24.0,
                output_gain_db,
                move |v| Message::Stage(idx, StageMessage::Nam(NamMessage::OutputGainChanged(v))),
                |v| format!("{v:+.1} dB"),
                0.1,
            ),
            labeled_slider(
                tr!(nam_mix),
                0.0..=1.0,
                mix,
                move |v| Message::Stage(idx, StageMessage::Nam(NamMessage::MixChanged(v))),
                |v| format!("{:.0}%", v * 100.0),
                0.01,
            ),
        ]
        .spacing(SPACING_TIGHT)
        .into()
    })
}
