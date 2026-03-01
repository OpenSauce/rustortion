use iced::widget::{button, column, container, pick_list, row, scrollable, space, text};
use iced::{Alignment, Color, Element, Length};

use crate::gui::components::widgets::common::{
    BORDER_RADIUS_CARD, BORDER_RADIUS_DIALOG, COLOR_MUTED, COLOR_SUCCESS, COLOR_WARNING,
    PADDING_NORMAL, SPACING_NORMAL, SPACING_TIGHT, TEXT_SIZE_INFO, TEXT_SIZE_LABEL,
};
use crate::tr;

use super::{DIALOG_TITLE_ROW_SPACING, DIALOG_TITLE_SIZE};

/// Standard dialog title row: title text + spacer + close button.
pub fn dialog_title_row<'a, M: Clone + 'a>(title: &'a str, close_msg: M) -> Element<'a, M> {
    row![
        text(title)
            .size(DIALOG_TITLE_SIZE)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            }),
        space::horizontal(),
        button(tr!(close)).on_press(close_msg),
    ]
    .spacing(DIALOG_TITLE_ROW_SPACING)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Outer dialog container with background + rounded border + width(2).
pub fn dialog_container<'a, M: 'a>(content: Element<'a, M>) -> Element<'a, M> {
    container(content)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(
                    iced::Border::default()
                        .rounded(BORDER_RADIUS_DIALOG)
                        .width(2),
                )
        })
        .into()
}

/// Dark semi-transparent sub-section container (used in settings, MIDI, hotkey dialogs).
pub fn dialog_section_container<'a, M: 'a>(content: Element<'a, M>) -> Element<'a, M> {
    container(content)
        .style(|_theme: &iced::Theme| {
            container::Style::default()
                .background(Color::from_rgba(0.0, 0.0, 0.0, 0.2))
                .border(iced::Border::default().rounded(BORDER_RADIUS_CARD))
        })
        .width(Length::Fill)
        .into()
}

/// Yellow "press any key" / "press MIDI device" waiting container.
pub fn waiting_for_input_view<'a, M: 'a>(prompt_text: &'a str) -> Element<'a, M> {
    container(
        text(prompt_text)
            .size(TEXT_SIZE_LABEL)
            .style(|_: &iced::Theme| iced::widget::text::Style {
                color: Some(COLOR_WARNING),
            }),
    )
    .padding(PADDING_NORMAL)
    .style(|_: &iced::Theme| {
        container::Style::default()
            .background(Color::from_rgba(1.0, 0.8, 0.0, 0.1))
            .border(iced::Border::default().rounded(BORDER_RADIUS_CARD))
    })
    .width(Length::Fill)
    .into()
}

/// Green "captured: X" container with preset picker + confirm button.
pub fn input_captured_view<'a, M: Clone + 'a>(
    description: &str,
    presets: &[String],
    selected_preset: Option<String>,
    on_select: impl Fn(String) -> M + 'a,
    confirm_msg: M,
) -> Element<'a, M> {
    let captured_text = text(format!("{} {}", tr!(captured), description))
        .size(TEXT_SIZE_LABEL)
        .style(|_: &iced::Theme| iced::widget::text::Style {
            color: Some(COLOR_SUCCESS),
        });

    let has_preset = selected_preset.is_some();

    let preset_picker = row![
        text(tr!(assign_to)).width(Length::Fixed(80.0)),
        pick_list(presets.to_vec(), selected_preset, on_select)
            .width(Length::Fill)
            .placeholder(tr!(select_preset)),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center);

    let confirm_button = if has_preset {
        button(tr!(confirm_mapping))
            .on_press(confirm_msg)
            .style(iced::widget::button::success)
    } else {
        button(tr!(confirm_mapping)).style(iced::widget::button::secondary)
    };

    container(column![captured_text, preset_picker, confirm_button,].spacing(SPACING_NORMAL))
        .padding(PADDING_NORMAL)
        .style(|_: &iced::Theme| {
            container::Style::default()
                .background(Color::from_rgba(0.0, 1.0, 0.0, 0.05))
                .border(iced::Border::default().rounded(BORDER_RADIUS_CARD))
        })
        .width(Length::Fill)
        .into()
}

/// Scrollable list of `description -> preset_name [x]` rows.
/// Takes owned `Vec<(description, preset_name)>` pairs so it's decoupled from domain types.
pub fn mapping_list_view<'a, M: Clone + 'a>(
    mappings: Vec<(String, String)>,
    empty_text: &'a str,
    on_remove: impl Fn(usize) -> M + 'a,
) -> Element<'a, M> {
    if mappings.is_empty() {
        muted_text(empty_text).into()
    } else {
        let mut col = column![].spacing(SPACING_TIGHT);

        for (idx, (desc, preset)) in mappings.into_iter().enumerate() {
            let mapping_row = row![
                text(desc).size(TEXT_SIZE_INFO).width(Length::Fixed(120.0)),
                text("\u{2192}")
                    .size(TEXT_SIZE_INFO)
                    .width(Length::Fixed(30.0)),
                text(preset).size(TEXT_SIZE_INFO).width(Length::Fill),
                button("\u{00d7}")
                    .on_press(on_remove(idx))
                    .style(iced::widget::button::danger)
                    .width(Length::Fixed(30.0)),
            ]
            .spacing(SPACING_NORMAL)
            .align_y(Alignment::Center);

            col = col.push(mapping_row);
        }

        scrollable(col).height(Length::Fixed(120.0)).into()
    }
}

/// Gray muted text for empty states.
pub fn muted_text(label: &str) -> iced::widget::Text<'_> {
    text(label)
        .size(TEXT_SIZE_INFO)
        .style(|_: &iced::Theme| iced::widget::text::Style {
            color: Some(COLOR_MUTED),
        })
}
