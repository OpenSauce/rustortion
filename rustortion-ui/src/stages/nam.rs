use iced::widget::{button, column, container, pick_list, row, rule, text};
use iced::{Alignment, Element, Length, Padding};

use rustortion_core::amp::stages::nam::NamConfig;
use rustortion_core::nam::registry;

use crate::components::widgets::common::{
    COLOR_SUBTLE, COLOR_WARNING, SPACING_NORMAL, SPACING_TIGHT, SPACING_WIDE, StageViewState, TEXT_SIZE_SMALL,
    labeled_slider, stage_card,
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
    // Lets a cab-inclusive model say whether the IR is a second cab or already out.
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

        // Extracted once at load, so this is cheap per frame.
        let model_info = model_name.as_deref().and_then(registry::info);

        // Filenames in the wild are often cryptic while the metadata names the real
        // amp. A field the file doesn't carry gets no row.
        let metadata_rows: Vec<(&str, String)> = model_info.as_deref().map_or_else(
            Vec::new,
            |info| {
                let mut rows: Vec<(&str, String)> = Vec::new();
                if let Some(gear) = info.gear.as_deref() {
                    rows.push((tr!(nam_gear), gear.to_owned()));
                }
                if let Some(tone) = info.tone_type.as_deref() {
                    rows.push((tr!(nam_tone_type), tone.to_owned()));
                }
                if let Some(by) = info.modeled_by.as_deref() {
                    rows.push((tr!(nam_modeled_by), by.to_owned()));
                }
                // Spans >20 dB across models — the volume jump when switching.
                if let Some(lufs) = info.loudness_lufs {
                    rows.push((tr!(nam_loudness), format!("{lufs:.1} LUFS")));
                }
                rows
            },
        );

        // Smaller and dimmer than the controls: it describes the file rather than
        // doing anything. Omitted when the file says nothing.
        let metadata_section: Element<'_, Message> = if metadata_rows.is_empty() {
            column![].into()
        } else {
            let rows = metadata_rows.into_iter().map(|(label, value)| {
                row![
                    text(format!("{label}:"))
                        .size(TEXT_SIZE_SMALL)
                        .style(|_| iced::widget::text::Style {
                            color: Some(COLOR_SUBTLE),
                        })
                        .width(Length::FillPortion(4)),
                    text(value)
                        .size(TEXT_SIZE_SMALL)
                        .width(Length::FillPortion(6)),
                ]
                .spacing(SPACING_TIGHT)
                .into()
            });

            column![
                rule::horizontal(1),
                text(format!("{}:", tr!(nam_metadata)))
                    .size(TEXT_SIZE_SMALL)
                    .style(|_| iced::widget::text::Style {
                        color: Some(COLOR_SUBTLE),
                    }),
                container(column(rows).spacing(2))
                    .padding(Padding::ZERO.left(SPACING_WIDE)),
            ]
            .spacing(SPACING_TIGHT)
            .into()
        };

        // `gear_type` is never displayed — only the conclusion drawn from it, the
        // live IR state, and a toggle. Always shown: the pairing is what matters, so
        // hiding it when the model says nothing just leaves the IR state a mystery.
        // Warning colour marks the two mismatched combinations; the rest stay quiet.
        let (cab_text, needs_attention) = match model_info
            .as_deref()
            .map(|info| (info.includes_cab, info.cab_from_name))
        {
            Some((Some(true), from_name)) => {
                // A name-derived conclusion is a guess; don't present it as fact.
                let source = if from_name {
                    format!(" ({})", tr!(nam_cab_from_name))
                } else {
                    String::new()
                };
                let cab = format!("{}{source}", tr!(nam_cab_included));
                if ir_bypassed {
                    (format!("{cab} · {}", tr!(nam_cab_ir_bypassed)), false)
                } else {
                    (format!("{cab} · {}", tr!(nam_cab_ir_conflict)), true)
                }
            }
            Some((Some(false), _)) => {
                let cab = tr!(nam_cab_not_included);
                if ir_bypassed {
                    (format!("{cab} · {}", tr!(nam_ir_recommended)), true)
                } else {
                    (format!("{cab} · {}", tr!(nam_ir_active)), false)
                }
            }
            // Nothing known about the model: report the IR state without judging it.
            Some((None, _)) | None => (
                if ir_bypassed {
                    tr!(nam_cab_ir_bypassed).to_owned()
                } else {
                    tr!(nam_ir_active).to_owned()
                },
                false,
            ),
        };

        let cab_line = row![
            text(cab_text)
                .style(move |_| iced::widget::text::Style {
                    color: Some(if needs_attention {
                        COLOR_WARNING
                    } else {
                        COLOR_SUBTLE
                    }),
                })
                .width(Length::Fill),
            // Goes out as a manual toggle, which is what it is — that clears the
            // auto-bypass flag, so we stop moving the control once the user has.
            button(text(if ir_bypassed {
                tr!(nam_enable_ir)
            } else {
                tr!(nam_bypass_ir)
            }))
            .on_press(Message::IrBypassed(!ir_bypassed)),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);

        // Rescan picks up newly dropped `.nam` files without restarting the host.
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

        // The cab/IR line stays with the controls: it reports a change to the live
        // signal chain, not a description of the file.
        column![
            model_selector,
            folder_row,
            info_line,
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
            metadata_section,
        ]
        .spacing(SPACING_TIGHT)
        .into()
    })
}
