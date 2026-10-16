use iced::widget::{button, column, container, responsive, row, rule, text};
use iced::{Alignment, Border, Color, Element, Length, Padding};
use iced_aw::menu::{self, Item, Menu, MenuBar};

use rustortion_core::amp::stages::nam::NamConfig;
use rustortion_core::nam::registry;

use crate::components::widgets::common::{
    BORDER_RADIUS_CARD, COLOR_SUBTLE, COLOR_WARNING, PADDING_NORMAL, PADDING_SMALL, SPACING_NORMAL,
    SPACING_TIGHT, SPACING_WIDE, StageViewState, TEXT_SIZE_SMALL, labeled_slider, stage_card,
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
    /// The picker's root button was pressed. The menu bar opens itself on the
    /// release; this exists only so the button has a press handler.
    MenuOpened,
}

// --- Model tree ---

/// One entry of the model picker menu: a folder that fans out into its contents, or
/// a model that can be selected. Built from the registry's flat `Folder/Model` keys.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelNode {
    Folder {
        label: String,
        children: Vec<Self>,
    },
    Model {
        /// Shown in the menu: the file's own name.
        label: String,
        /// Stored in the preset and looked up in the registry: the full key.
        key: String,
    },
}

impl ModelNode {
    fn label(&self) -> &str {
        match self {
            Self::Folder { label, .. } | Self::Model { label, .. } => label,
        }
    }
}

/// The last path segment of a registry key: what the closed picker shows, since
/// the folder is visible in the menu and the full key would not fit the card.
fn leaf_label(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

/// Group registry keys into a folder tree. Folders and loose files sort together
/// by name, ignoring case, at every level.
fn model_tree(keys: &[String]) -> Vec<ModelNode> {
    let mut root = Vec::new();
    for key in keys {
        let mut segments = key.split('/').peekable();
        let mut level = &mut root;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                level.push(ModelNode::Model {
                    label: segment.to_owned(),
                    key: key.clone(),
                });
                break;
            }
            let position = level
                .iter()
                .position(|node| matches!(node, ModelNode::Folder { label, .. } if label == segment))
                .unwrap_or_else(|| {
                    level.push(ModelNode::Folder {
                        label: segment.to_owned(),
                        children: Vec::new(),
                    });
                    level.len() - 1
                });
            let ModelNode::Folder { children, .. } = &mut level[position] else {
                unreachable!("position was found by matching a Folder");
            };
            level = children;
        }
    }
    sort_tree(&mut root);
    root
}

fn sort_tree(nodes: &mut [ModelNode]) {
    nodes.sort_by_cached_key(|node| node.label().to_lowercase());
    for node in nodes {
        if let ModelNode::Folder { children, .. } = node {
            sort_tree(children);
        }
    }
}

/// Pointing right: this entry fans out into a child menu.
const MENU_CARET_RIGHT: &str = "\u{25B8}";
/// Pointing down: the closed picker, as a pick list draws it.
const MENU_CARET_DOWN: &str = "\u{25BE}";
/// Wide enough for the long capture names packs use without wrapping every row.
const MENU_WIDTH: f32 = 380.0;

type MenuItem<'a> = Item<'a, Message, iced::Theme, iced::Renderer>;
type ModelMenu<'a> = Menu<'a, Message, iced::Theme, iced::Renderer>;

/// A child menu, sized and spaced the same at every depth.
fn model_menu(items: Vec<MenuItem<'_>>) -> ModelMenu<'_> {
    Menu::new(items)
        .width(MENU_WIDTH)
        .offset(PADDING_SMALL)
        .spacing(2.0)
}

/// One selectable row. The selected model is marked in the warning colour rather
/// than a check mark, which would shift every other label over by a column.
fn menu_entry<'a>(label: String, selected: bool, on_press: Message) -> Element<'a, Message> {
    button(text(label).style(move |theme: &iced::Theme| iced::widget::text::Style {
        color: Some(if selected { COLOR_WARNING } else { theme.palette().text }),
    }))
    .width(Length::Fill)
    .padding([PADDING_SMALL, PADDING_NORMAL])
    .style(menu_row_style)
    .on_press(on_press)
    .into()
}

/// A folder row. It has no press action: hovering opens the child menu, and the
/// menu bar draws the highlight for the open path itself.
fn menu_folder<'a>(label: String) -> Element<'a, Message> {
    button(
        row![text(label).width(Length::Fill), text(MENU_CARET_RIGHT)].spacing(SPACING_TIGHT),
    )
    .width(Length::Fill)
    .padding([PADDING_SMALL, PADDING_NORMAL])
    .style(menu_row_style)
    .into()
}

/// Flat and transparent at rest, so a column of rows reads as one list rather than
/// a stack of buttons. Disabled is the folder rows' resting state, not an error.
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

/// Menu items for one level of the tree, recursing into folders.
fn menu_items<'a>(
    nodes: &[ModelNode],
    idx: usize,
    selected: Option<&str>,
) -> Vec<MenuItem<'a>> {
    nodes
        .iter()
        .map(|node| match node {
            ModelNode::Folder { label, children } => Item::with_menu(
                menu_folder(label.clone()),
                model_menu(menu_items(children, idx, selected)),
            )
            // The bar closes on any item click by default, and people do click a
            // folder to open it. Only picking a model, or clicking away, closes.
            .close_on_click(false),
            ModelNode::Model { label, key } => Item::new(menu_entry(
                label.clone(),
                selected == Some(key.as_str()),
                Message::Stage(
                    idx,
                    StageMessage::Nam(NamMessage::ModelSelected(Some(key.clone()))),
                ),
            )),
        })
        .collect()
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
        NamMessage::Rescan => Some(ParamUpdate::RescanNamModels),
        NamMessage::OpenFolder => Some(ParamUpdate::OpenNamModelsDir),
        NamMessage::MenuOpened => None,
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
        // The picker is a one-root menu bar rather than a pick list: a pick list is a
        // flat list, and packs ship as folders of dozens of files. Each folder fans
        // out sideways; loose files sit alongside the folders at the same level.
        //
        // The bar lays its root out against an unbounded width, so a `Fill` button
        // inside it never gets a size. `responsive` hands over the width the row
        // actually allotted, and the button takes exactly that.
        let tree = model_tree(&registry::available_names());
        let picker_model = model_name.clone();
        let closed_label = model_name
            .as_deref()
            .map_or_else(|| tr!(nam_no_model).to_owned(), |key| leaf_label(key).to_owned());
        let model_picker = responsive(move |size| {
            // "(None)" first so a selected model can be cleared back to passthrough.
            let mut items = vec![Item::new(menu_entry(
                tr!(nam_no_model).to_owned(),
                picker_model.is_none(),
                Message::Stage(idx, StageMessage::Nam(NamMessage::ModelSelected(None))),
            ))];
            items.extend(menu_items(&tree, idx, picker_model.as_deref()));

            MenuBar::new(vec![Item::with_menu(
                button(
                    row![
                        text(closed_label.clone()).width(Length::Fill),
                        text(MENU_CARET_DOWN),
                    ]
                    .spacing(SPACING_TIGHT),
                )
                .width(size.width)
                .style(button::secondary)
                // Opening is handled by the menu bar; the press itself does nothing,
                // but a button without `on_press` renders as disabled.
                .on_press(Message::Stage(idx, StageMessage::Nam(NamMessage::MenuOpened))),
                // As wide as the button, so it hangs straight below it like a pick
                // list. Left free, the bar would right-align it whenever the button
                // sits in the right half of the window.
                model_menu(items).width(size.width),
            )])
            // Behave like a pick list: stay open until something is clicked.
            .close_on_item_click_global(true)
            .close_on_background_click_global(true)
            // Leaving the menu must not close it either. The bar pads its bounds
            // by this margin and tests containment, and `width + 2 * MAX`
            // overflows to infinity, so the cursor is never outside. A large
            // finite value would depend on the monitor size.
            .safe_bounds_margin(f32::MAX)
            // Highlights the folder whose child menu is open.
            .draw_path(menu::DrawPath::Backdrop)
            .into()
        })
        .width(Length::FillPortion(7))
        .height(Length::Shrink);

        let model_selector = row![
            text(tr!(nam_model)).width(Length::FillPortion(3)),
            model_picker,
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
                if let Some(dbfs) = info.loudness_dbfs {
                    rows.push((tr!(nam_loudness), format!("{dbfs:.1} dBFS")));
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
            // The IR bypass is never changed automatically; this button is the only
            // thing on this card that touches it, so the write is always a real user
            // gesture — which is what a plugin host records as automation.
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
        let dir_text = models_dir.as_deref().map_or_else(
            || format!("{}: —", tr!(nam_models_dir)),
            |dir| format!("{}: {dir}", tr!(nam_models_dir)),
        );
        // With no directory the row shows "—", so the button has nothing to open:
        // leave `on_press` unset and iced renders it disabled, rather than offering
        // a click whose only effect is a log line.
        let open_folder = button(text(tr!(nam_open_folder)));
        let open_folder = if models_dir.is_some() {
            open_folder.on_press(Message::Stage(
                idx,
                StageMessage::Nam(NamMessage::OpenFolder),
            ))
        } else {
            open_folder
        };
        let folder_row = row![
            text(dir_text).width(Length::Fill),
            open_folder,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(node: &ModelNode) -> (&str, &[ModelNode]) {
        match node {
            ModelNode::Folder { label, children } => (label.as_str(), children.as_slice()),
            ModelNode::Model { .. } => panic!("expected a folder, got {node:?}"),
        }
    }

    fn model(node: &ModelNode) -> (&str, &str) {
        match node {
            ModelNode::Model { label, key } => (label.as_str(), key.as_str()),
            ModelNode::Folder { .. } => panic!("expected a model, got {node:?}"),
        }
    }

    /// Registry keys are `Folder/Sub/Model`. The menu groups them by folder, keeps
    /// the full key on each leaf (that is what gets selected), and sorts folders
    /// and loose files together by name, ignoring case, at every level.
    #[test]
    fn model_tree_groups_keys_by_folder() {
        // `Pack` is both a folder and a loose file: both must survive, side by side.
        let names = [
            "Pack/Deep/leaf",
            "Pack/inner",
            "alpha",
            "Zeta/one",
            "beta",
            "Pack",
        ]
        .map(str::to_owned);

        let tree = model_tree(&names);

        let labels: Vec<&str> = tree.iter().map(ModelNode::label).collect();
        assert_eq!(labels, ["alpha", "beta", "Pack", "Pack", "Zeta"]);
        assert_eq!(model(&tree[0]), ("alpha", "alpha"));
        assert_eq!(model(&tree[3]), ("Pack", "Pack"));

        let (_, pack) = folder(&tree[2]);
        assert_eq!(pack.len(), 2);
        let (deep_label, deep) = folder(&pack[0]);
        assert_eq!(deep_label, "Deep");
        assert_eq!(model(&deep[0]), ("leaf", "Pack/Deep/leaf"));
        assert_eq!(model(&pack[1]), ("inner", "Pack/inner"));
    }

    /// What the closed picker shows: only the file's own name, since the folder is
    /// visible in the menu and the full key would not fit the card.
    #[test]
    fn leaf_label_drops_the_folder_prefix() {
        assert_eq!(leaf_label("Pack/Deep/leaf"), "leaf");
        assert_eq!(leaf_label("alpha"), "alpha");
    }
}
