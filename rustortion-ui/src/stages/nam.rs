use iced::widget::{button, column, pick_list, row, text};
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
    /// Re-scan the NAM models directory and refresh the model pick-list.
    Rescan,
    /// Reveal the NAM models directory in the file manager.
    OpenFolder,
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
            // Selecting a model is a non-float change: rebuild the stage. The
            // NAM-specific variant also lets the app reconcile the IR cabinet, since
            // the new model may already contain a cab.
            Some(ParamUpdate::NamModelSelected)
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
        NamMessage::Rescan => Some(ParamUpdate::RescanNamModels),
        NamMessage::OpenFolder => Some(ParamUpdate::OpenNamModelsDir),
    }
}

// --- View ---

pub fn view(idx: usize, cfg: &NamConfig, state: StageViewState) -> Element<'_, Message> {
    let model_name = cfg.model_name.clone();
    let input_gain_db = cfg.input_gain_db;
    let output_gain_db = cfg.output_gain_db;
    let mix = cfg.mix;
    // The folder where `.nam` files live, shown so users know where to drop models.
    let models_dir = state
        .nam_models_dir
        .as_ref()
        .map(|p| p.display().to_string());
    // Effective engine rate, read out before `state` is moved into the card closure.
    let engine_rate = state.engine_sample_rate;
    // Whether the IR cabinet is currently bypassed, so a cab-inclusive model can say
    // whether the IR after it is a second cab or already out of the way.
    let ir_bypassed = state.ir_bypassed;

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
        let info_line: Element<'_, Message> = match model_name.as_deref() {
            Some(name) => match registry::get(name) {
                Some(model) => {
                    let native_rate = model.expected_sample_rate() as u32;
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

        // Cached summary of the selected model's metadata. Cheap by construction:
        // extracted once at load, so this runs per frame without re-parsing JSON.
        let model_info = model_name.as_deref().and_then(registry::info);

        // What the model actually is, when its metadata says. Filenames in the wild
        // are often cryptic ("S-[PRE] Divine Sheep #02") while the metadata names
        // the real amp, so this is frequently the only place to see it.
        let details: Vec<Element<'_, Message>> = model_info
            .as_deref()
            .filter(|info| !info.is_empty())
            .map(|info| {
                let mut lines: Vec<Element<'_, Message>> = Vec::new();

                // Gear and tone belong together: "Marshall JMP-50 · overdrive".
                let gear_parts: Vec<&str> = [info.gear.as_deref(), info.tone_type.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                if !gear_parts.is_empty() {
                    lines.push(text(gear_parts.join(" · ")).into());
                }

                // Attribution and level. Loudness is worth the space: it spans more
                // than 20 dB across models in circulation, which is exactly the
                // volume jump you hear when switching between them.
                let mut credit = Vec::new();
                if let Some(by) = info.modeled_by.as_deref() {
                    credit.push(format!("{}: {by}", tr!(nam_modeled_by)));
                }
                if let Some(lufs) = info.loudness_lufs {
                    credit.push(format!("{lufs:.1} LUFS"));
                }
                if !credit.is_empty() {
                    lines.push(text(credit.join(" · ")).into());
                }

                lines
            })
            .unwrap_or_default();

        // Whether the selected model already contains a speaker cab, per its own
        // metadata. `None` means the file doesn't say (or says something we don't
        // recognise) — in that case we show nothing rather than guess.
        let cab_line: Element<'_, Message> = match model_info.and_then(|info| info.includes_cab) {
            Some(true) => {
                // The IR is a second cab unless it's bypassed. When it is bypassed,
                // say so — otherwise the automatic bypass looks like a bug.
                let detail = if ir_bypassed {
                    tr!(nam_cab_ir_bypassed)
                } else {
                    tr!(nam_cab_ir_conflict)
                };
                text(format!("{} · {detail}", tr!(nam_cab_included))).into()
            }
            Some(false) => text(tr!(nam_cab_not_included)).into(),
            None => text(String::new()).into(),
        };

        // Folder location, a button to open it in the file manager, and a Rescan so
        // users can drop new `.nam` files and pick them up without restarting the host.
        let dir_text = models_dir.map_or_else(
            || format!("{}: —", tr!(nam_models_dir)),
            |dir| format!("{}: {dir}", tr!(nam_models_dir)),
        );
        let folder_row = row![
            text(dir_text).width(Length::Fill),
            button(text(tr!(nam_open_folder))).on_press(Message::Stage(
                idx,
                StageMessage::Nam(NamMessage::OpenFolder)
            )),
            button(text(tr!(nam_rescan_models)))
                .on_press(Message::Stage(idx, StageMessage::Nam(NamMessage::Rescan))),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);

        // `details` is variable-length (a model may carry all, some, or none of the
        // descriptive fields), so the card is assembled rather than written as one
        // literal: fixed rows, then the details, then the rest.
        let header = column![model_selector, folder_row, info_line]
            .spacing(SPACING_TIGHT)
            .extend(details);

        column![
            header,
            cab_line,
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
