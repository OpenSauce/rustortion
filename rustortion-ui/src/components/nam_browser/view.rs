//! The NAM tab: every model in a list on the left, the selected NAM stage on the right.

use std::path::PathBuf;

use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, rule, scrollable, text, text_input, tooltip};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use rustortion_core::amp::stages::nam::NamConfig;
use rustortion_core::nam::{ModelInfo, registry};

use super::NamBrowser;
use super::selection::{PaneState, nam_stage_indices, pane_state};
use super::tree::{ListRow, leaf_label, model_tree, visible_rows};
use crate::app::{tab_button_active, tab_button_inactive};
use crate::components::widgets::common::{
    BORDER_RADIUS_CARD, COLOR_MUTED, COLOR_SUBTLE, COLOR_WARNING, PADDING_NORMAL, PADDING_SMALL,
    SPACING_NORMAL, SPACING_TIGHT, TEXT_SIZE_SECTION_TITLE, TEXT_SIZE_SMALL, icon_button,
    labeled_slider, section_container,
};
use crate::messages::{Message, NamTabMessage};
use crate::stages::{NamMessage, StageConfig, StageMessage};
use crate::tr;

/// Extra left padding per folder level in the model list.
const INDENT_PER_LEVEL: f32 = 18.0;
/// Opacity of the `palette.primary` tint behind the loaded model's row.
const LOADED_ROW_ALPHA: f32 = 0.22;
/// Gap between list rows, so neighbouring hover highlights stay apart.
const ROW_SPACING: f32 = 2.0;

pub struct NamViewContext<'a> {
    pub stages: &'a [StageConfig],
    /// Device rate × oversampling: the rate stages are built at.
    pub engine_sample_rate: u32,
    pub models_dir: Option<PathBuf>,
    pub ir_bypassed: bool,
    /// The chain is at `DEFAULT_CHAIN_CAPACITY`.
    pub chain_full: bool,
}

impl NamBrowser {
    pub fn view<'a>(&'a self, ctx: &NamViewContext<'a>) -> Element<'a, Message> {
        let pane = pane_state(ctx.stages, self.focus, |k| registry::get(k).is_some());
        row![
            pane_card(self.list_pane(ctx, &pane), 9),
            pane_card(stage_pane(ctx, &pane), 11),
        ]
        .spacing(SPACING_NORMAL)
        .height(Length::Fill)
        .into()
    }

    fn list_pane<'a>(&'a self, ctx: &NamViewContext<'a>, pane: &PaneState) -> Element<'a, Message> {
        let catalog = registry::catalog();
        let dir = ctx.models_dir.as_ref().map(|p| p.display().to_string());

        let filter = text_input(tr!(nam_filter_placeholder), &self.filter)
            .on_input(|f| Message::Nam(NamTabMessage::FilterChanged(f)));

        let list = if catalog.is_empty() {
            empty_folder_hint(dir.as_deref())
        } else {
            let loaded = loaded_key(pane);
            // A click would add a NAM stage, and there is no room for one.
            let models_clickable = !(ctx.chain_full && *pane == PaneState::NoStage);
            let rows = visible_rows(
                &model_tree(&catalog),
                &self.filter,
                &self.open_folders,
                loaded,
            )
            .into_iter()
            .map(|r| list_row(r, loaded, ctx.engine_sample_rate, models_clickable));
            scrollable(column(rows).spacing(ROW_SPACING).width(Length::Fill))
                .spacing(SPACING_TIGHT)
                .height(Length::Fill)
                .into()
        };

        column![
            filter,
            list,
            rule::horizontal(1),
            list_footer(dir.as_deref(), catalog.len()),
        ]
        .spacing(SPACING_NORMAL)
        .height(Length::Fill)
        .into()
    }
}

/// One of the two side-by-side cards: `portion` of the row wide, full height.
fn pane_card(content: Element<'_, Message>, portion: u16) -> Element<'_, Message> {
    container(section_container(content))
        .width(Length::FillPortion(portion))
        .height(Length::Fill)
        .into()
}

/// The model the selected NAM stage names, whether or not it is registered.
fn loaded_key(pane: &PaneState) -> Option<&str> {
    match pane {
        PaneState::Loaded { key, .. } | PaneState::Missing { key, .. } => Some(key),
        PaneState::NoStage | PaneState::Empty { .. } => None,
    }
}

fn text_color(color: Color) -> impl Fn(&iced::Theme) -> text::Style {
    move |_| text::Style { color: Some(color) }
}

// --- Left pane ---

fn empty_folder_hint<'a>(dir: Option<&str>) -> Element<'a, Message> {
    container(
        column![
            text(tr!(nam_empty_folder_hint)),
            text(dir.unwrap_or("—").to_owned()).wrapping(Wrapping::WordOrGlyph),
        ]
        .spacing(SPACING_TIGHT),
    )
    .height(Length::Fill)
    .into()
}

fn list_footer<'a>(dir: Option<&str>, count: usize) -> Element<'a, Message> {
    let summary = format!("{} · {count} {}", dir.unwrap_or("—"), tr!(nam_models_count));
    // With no directory there is nothing to open, so the button renders disabled.
    let open_folder = button(text(tr!(nam_open_folder)))
        .style(button::secondary)
        .on_press_maybe(
            dir.is_some()
                .then_some(Message::Nam(NamTabMessage::OpenFolder)),
        );
    row![
        text(summary)
            .size(TEXT_SIZE_SMALL)
            .style(text_color(COLOR_SUBTLE))
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
        open_folder,
        button(text(tr!(nam_rescan_models)))
            .style(button::secondary)
            .on_press(Message::Nam(NamTabMessage::Rescan)),
    ]
    .spacing(SPACING_TIGHT)
    .align_y(Alignment::Center)
    .into()
}

fn list_row<'a>(
    row_data: ListRow,
    loaded: Option<&str>,
    engine_rate: u32,
    models_clickable: bool,
) -> Element<'a, Message> {
    match row_data {
        ListRow::Folder {
            depth,
            label,
            path,
            open,
        } => folder_row(depth, label, path, open),
        ListRow::Model {
            depth,
            label,
            key,
            folder,
            rate_hz,
        } => {
            let is_loaded = loaded == Some(key.as_str());
            let on_press =
                models_clickable.then_some(Message::Nam(NamTabMessage::ModelClicked(key)));
            model_row(
                depth,
                label,
                folder,
                rate_hz,
                engine_rate,
                is_loaded,
                on_press,
            )
        }
    }
}

fn folder_row<'a>(depth: usize, label: String, path: String, open: bool) -> Element<'a, Message> {
    let caret = if open { "\u{25BE}" } else { "\u{25B8}" };
    list_button(
        row![
            text(caret),
            text(label)
                .wrapping(Wrapping::WordOrGlyph)
                .width(Length::Fill),
        ]
        .spacing(SPACING_TIGHT),
        depth,
        false,
    )
    .on_press(Message::Nam(NamTabMessage::FolderToggled(path)))
    .into()
}

fn model_row<'a>(
    depth: usize,
    label: String,
    folder: Option<String>,
    rate_hz: u32,
    engine_rate: u32,
    is_loaded: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    // The engine passes a model at another rate through dry, so say so before the click.
    let rate_mismatch = rate_hz.abs_diff(engine_rate) > 1;

    let mut name = column![];
    if let Some(folder) = folder {
        name = name.push(
            text(folder)
                .size(TEXT_SIZE_SMALL)
                .style(text_color(COLOR_SUBTLE))
                .wrapping(Wrapping::WordOrGlyph),
        );
    }
    name = name.push(
        text(label)
            .wrapping(Wrapping::WordOrGlyph)
            .style(move |_| text::Style {
                color: rate_mismatch.then_some(COLOR_MUTED),
            }),
    );

    let mut content = row![name.width(Length::Fill)]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);
    if rate_mismatch {
        content = content.push(text(format_khz(rate_hz)).style(text_color(COLOR_WARNING)));
    }

    list_button(content, depth, is_loaded)
        .on_press_maybe(on_press)
        .into()
}

/// A flat, full-width list row, indented by folder depth.
fn list_button<'a>(
    content: impl Into<Element<'a, Message>>,
    depth: usize,
    is_loaded: bool,
) -> button::Button<'a, Message> {
    let indent = depth as f32 * INDENT_PER_LEVEL;
    button(content)
        .width(Length::Fill)
        .padding(Padding::from([PADDING_SMALL, PADDING_NORMAL]).left(PADDING_NORMAL + indent))
        .style(move |theme: &iced::Theme, status| {
            let style = menu_row_style(theme, status);
            if is_loaded {
                style.with_background(Color {
                    a: LOADED_ROW_ALPHA,
                    ..theme.palette().primary
                })
            } else {
                style
            }
        })
}

/// Flat and transparent at rest, so a column of rows reads as one list rather than
/// a stack of buttons. Disabled is a model row that cannot load because the chain is full.
fn menu_row_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base = button::Style {
        text_color: palette.background.base.text,
        border: Border::default().rounded(BORDER_RADIUS_CARD),
        ..button::Style::default()
    };
    match status {
        button::Status::Active | button::Status::Disabled => {
            base.with_background(Color::TRANSPARENT)
        }
        button::Status::Hovered => base.with_background(palette.background.strong.color),
        button::Status::Pressed => base.with_background(palette.primary.weak.color),
    }
}

/// `44100` → "44.1 kHz", `48000` → "48 kHz". Integer maths, so nothing rounds.
fn format_khz(rate_hz: u32) -> String {
    let (whole, frac) = (rate_hz / 1000, rate_hz % 1000);
    if frac == 0 {
        format!("{whole} kHz")
    } else {
        let frac = format!("{frac:03}");
        format!("{whole}.{} kHz", frac.trim_end_matches('0'))
    }
}

// --- Right pane ---

fn stage_pane<'a>(ctx: &NamViewContext<'a>, pane: &PaneState) -> Element<'a, Message> {
    let (idx, key) = match pane {
        PaneState::NoStage => return no_stage_message(ctx.chain_full),
        PaneState::Empty { idx } => (*idx, None),
        PaneState::Loaded { idx, key } | PaneState::Missing { idx, key } => {
            (*idx, Some(key.as_str()))
        }
    };
    // `pane_state` only ever names a NAM stage.
    let Some(stage @ StageConfig::Nam(cfg)) = ctx.stages.get(idx) else {
        return no_stage_message(ctx.chain_full);
    };
    let title = key.map_or_else(
        || tr!(nam_no_model_loaded).to_owned(),
        |k| leaf_label(k).to_owned(),
    );
    // Extracted once at load, so this is cheap per frame.
    let info = key.and_then(registry::info);
    let native_rate = match pane {
        PaneState::Loaded { key, .. } => {
            registry::get(key).map(|model| model.expected_sample_rate() as u32)
        }
        _ => None,
    };

    let mut content = column![].spacing(SPACING_NORMAL).width(Length::Fill);
    if let Some(switcher) = stage_switcher(ctx.stages, idx) {
        content = content.push(switcher);
    }
    content = content.push(stage_header(idx, title, stage.bypassed()));
    if matches!(pane, PaneState::Missing { .. }) {
        content = content.push(text(tr!(nam_model_not_found)).style(text_color(COLOR_WARNING)));
    }
    // A mismatched model is bypassed (dry passthrough, no resampling): show both rates.
    if let Some(native) = native_rate
        && native.abs_diff(ctx.engine_sample_rate) > 1
    {
        content = content.push(
            text(format!(
                "{}: {native} Hz · {} Hz",
                tr!(nam_rate_mismatch_bypassed),
                ctx.engine_sample_rate
            ))
            .style(text_color(COLOR_WARNING)),
        );
    }
    content = content
        .push(rule::horizontal(1))
        .push(cab_line(info.as_deref(), ctx.ir_bypassed))
        .push(rule::horizontal(1))
        .push(gain_sliders(idx, cfg));
    if let Some(native) = native_rate {
        content = content
            .push(rule::horizontal(1))
            .push(detail_rows(info.as_deref(), native));
    }

    scrollable(content)
        .spacing(SPACING_TIGHT)
        .height(Length::Fill)
        .into()
}

fn no_stage_message<'a>(chain_full: bool) -> Element<'a, Message> {
    let hint = if chain_full {
        tr!(nam_chain_full)
    } else {
        tr!(nam_click_to_load)
    };
    container(
        column![
            text(tr!(nam_no_model_loaded)).size(TEXT_SIZE_SECTION_TITLE),
            text(hint)
                .style(text_color(COLOR_SUBTLE))
                .align_x(Alignment::Center),
        ]
        .spacing(SPACING_TIGHT)
        .align_x(Alignment::Center),
    )
    .center(Length::Fill)
    .into()
}

/// One button per NAM stage, or nothing when there is only one to pick.
fn stage_switcher<'a>(stages: &[StageConfig], selected: usize) -> Option<Element<'a, Message>> {
    let indices = nam_stage_indices(stages);
    if indices.len() <= 1 {
        return None;
    }
    let buttons = indices.into_iter().map(|idx| {
        let model = match stages.get(idx) {
            Some(StageConfig::Nam(cfg)) => cfg.model_name.as_deref().map(leaf_label),
            _ => None,
        }
        .unwrap_or_else(|| tr!(nam_no_model_loaded));
        button(
            text(format!("{} {} · {model}", tr!(stage_nam), idx + 1))
                .wrapping(Wrapping::WordOrGlyph),
        )
        .width(Length::Fill)
        .padding([PADDING_SMALL, PADDING_NORMAL])
        .style(if idx == selected {
            tab_button_active
        } else {
            tab_button_inactive
        })
        .on_press(Message::Nam(NamTabMessage::StageFocused(idx)))
        .into()
    });
    Some(row(buttons).spacing(SPACING_TIGHT).into())
}

fn stage_header<'a>(idx: usize, title: String, bypassed: bool) -> Element<'a, Message> {
    let bypass = tooltip(
        icon_button(
            "⏻",
            Some(Message::ToggleStageBypass(idx)),
            if bypassed {
                button::secondary
            } else {
                button::success
            },
        ),
        tr!(stage_bypass_tooltip),
        tooltip::Position::Bottom,
    );
    row![
        bypass,
        text(title)
            .size(TEXT_SIZE_SECTION_TITLE)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
        icon_button("×", Some(Message::RemoveStage(idx)), button::danger),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .into()
}

/// The model's cab set against the live IR state, with the one button that toggles the IR.
fn cab_line<'a>(info: Option<&ModelInfo>, ir_bypassed: bool) -> Element<'a, Message> {
    // `gear_type` is never displayed — only the conclusion drawn from it, the
    // live IR state, and a toggle. Always shown: the pairing is what matters, so
    // hiding it when the model says nothing just leaves the IR state a mystery.
    // Warning colour marks the two mismatched combinations; the rest stay quiet.
    let (cab_text, needs_attention) = match info.map(|info| (info.includes_cab, info.cab_from_name))
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

    row![
        text(cab_text)
            .style(text_color(if needs_attention {
                COLOR_WARNING
            } else {
                COLOR_SUBTLE
            }))
            .width(Length::Fill),
        // The IR bypass is never changed automatically; this button is the only
        // NAM control that touches it, so the write is always a real user
        // gesture — which is what a plugin host records as automation.
        button(text(if ir_bypassed {
            tr!(nam_enable_ir)
        } else {
            tr!(nam_bypass_ir)
        }))
        .on_press(Message::IrBypassed(!ir_bypassed)),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .into()
}

fn gain_sliders<'a>(idx: usize, cfg: &NamConfig) -> Element<'a, Message> {
    let nam = move |msg| Message::Stage(idx, StageMessage::Nam(msg));
    column![
        labeled_slider(
            tr!(nam_input_gain),
            -24.0..=24.0,
            cfg.input_gain_db,
            move |v| nam(NamMessage::InputGainChanged(v)),
            |v| format!("{v:+.1} dB"),
            0.1,
        ),
        labeled_slider(
            tr!(nam_output_gain),
            -24.0..=24.0,
            cfg.output_gain_db,
            move |v| nam(NamMessage::OutputGainChanged(v)),
            |v| format!("{v:+.1} dB"),
            0.1,
        ),
        labeled_slider(
            tr!(nam_mix),
            0.0..=1.0,
            cfg.mix,
            move |v| nam(NamMessage::MixChanged(v)),
            |v| format!("{:.0}%", v * 100.0),
            0.01,
        ),
    ]
    .spacing(SPACING_TIGHT)
    .into()
}

/// What the file says about itself. A field it lacks gets no row.
fn detail_rows<'a>(info: Option<&ModelInfo>, native_rate: u32) -> Element<'a, Message> {
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(info) = info {
        if let Some(gear) = &info.gear {
            rows.push((tr!(nam_gear), gear.clone()));
        }
        if let Some(tone) = &info.tone_type {
            rows.push((tr!(nam_tone_type), tone.clone()));
        }
        if let Some(by) = &info.modeled_by {
            rows.push((tr!(nam_modeled_by), by.clone()));
        }
        if let Some(dbfs) = info.loudness_dbfs {
            rows.push((tr!(nam_loudness), format!("{dbfs:.1} dBFS")));
        }
    }
    rows.push((tr!(nam_native_rate), format!("{native_rate} Hz")));

    column(rows.into_iter().map(|(label, value)| {
        row![
            text(label)
                .size(TEXT_SIZE_SMALL)
                .style(text_color(COLOR_SUBTLE))
                .width(Length::FillPortion(3)),
            text(value)
                .size(TEXT_SIZE_SMALL)
                .wrapping(Wrapping::WordOrGlyph)
                .width(Length::FillPortion(7)),
        ]
        .spacing(SPACING_NORMAL)
        .into()
    }))
    .spacing(ROW_SPACING)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_shown_in_khz_without_trailing_zeros() {
        assert_eq!(format_khz(44_100), "44.1 kHz");
        assert_eq!(format_khz(48_000), "48 kHz");
        assert_eq!(format_khz(96_000), "96 kHz");
        assert_eq!(format_khz(88_200), "88.2 kHz");
    }
}
