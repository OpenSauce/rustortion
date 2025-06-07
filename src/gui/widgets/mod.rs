pub mod compressor;
pub mod filter;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

use crate::gui::amp::Message;
use iced::widget::{button, row, text};
use iced::{Element, Font, Length};

pub fn labeled_slider<'a, F: 'a + Fn(f32) -> Message>(
    label: &'a str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    on_change: F,
    format: impl Fn(f32) -> String + 'a,
    step: f32,
) -> Element<'a, Message> {
    use iced::Alignment;
    use iced::widget::{row, slider, text};

    row![
        text(label).width(Length::FillPortion(3)),
        slider(range, value, on_change)
            .width(Length::FillPortion(5))
            .step(step),
        text(format(value)).width(Length::FillPortion(2)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

const ICONS_FONT: Font = Font::MONOSPACE;

pub fn icon_button<'a>(
    icon: &'a str,
    message: Option<Message>,
    style: fn(&iced::Theme, button::Status) -> iced::widget::button::Style,
) -> Element<'a, Message> {
    let btn = button(text(icon).font(ICONS_FONT))
        .width(Length::Fixed(30.0))
        .style(style);

    if let Some(msg) = message {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

pub fn stage_header(stage_name: &str, idx: usize, total_stages: usize) -> Element<Message> {
    let header_text = format!("{} {}", stage_name, idx + 1);

    let move_up_btn = if idx > 0 {
        icon_button(
            "↑",
            Some(Message::MoveStageUp(idx)),
            iced::widget::button::primary,
        )
    } else {
        icon_button("↑", None, iced::widget::button::secondary)
    };

    let move_down_btn = if idx < total_stages.saturating_sub(1) {
        icon_button(
            "↓",
            Some(Message::MoveStageDown(idx)),
            iced::widget::button::primary,
        )
    } else {
        icon_button("↓", None, iced::widget::button::secondary)
    };

    let remove_btn = icon_button(
        "×",
        Some(Message::RemoveStage(idx)),
        iced::widget::button::danger,
    );

    // Build the complete row
    row![move_up_btn, move_down_btn, remove_btn, text(header_text)]
        .spacing(5)
        .align_y(iced::Alignment::Center)
        .into()
}
