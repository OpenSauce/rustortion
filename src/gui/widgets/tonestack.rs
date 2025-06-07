use super::{icon_button, labeled_slider};
use crate::gui::amp::{Message, ToneStackConfig};
use crate::sim::stages::tonestack::ToneStackModel;
use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

const TONE_STACK_MODELS: [ToneStackModel; 4] = [
    ToneStackModel::Modern,
    ToneStackModel::British,
    ToneStackModel::American,
    ToneStackModel::Flat,
];

pub fn tonestack_widget(
    idx: usize,
    cfg: &ToneStackConfig,
    total_stages: usize,
) -> Element<Message> {
    let mut header = row![text(format!("Tonestack {}", idx + 1))].spacing(5);

    if idx > 0 {
        header = header.push(icon_button(
            "↑",
            Some(Message::MoveStageUp(idx)),
            iced::widget::button::primary,
        ));
    } else {
        header = header.push(icon_button("↑", None, iced::widget::button::secondary));
    }

    if idx < total_stages.saturating_sub(1) {
        header = header.push(icon_button(
            "↓",
            Some(Message::MoveStageDown(idx)),
            iced::widget::button::primary,
        ));
    } else {
        header = header.push(icon_button("↓", None, iced::widget::button::secondary));
    }

    header = header.push(icon_button(
        "×",
        Some(Message::RemoveStage(idx)),
        iced::widget::button::danger,
    ));

    let model_picker = row![
        text("Model:").width(Length::FillPortion(3)),
        pick_list(TONE_STACK_MODELS, Some(cfg.model), move |m| {
            Message::ToneStackModelChanged(idx, m)
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        model_picker,
        labeled_slider(
            "Bass",
            0.0..=1.0,
            cfg.bass,
            move |v| Message::ToneStackBassChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
        labeled_slider(
            "Mid",
            0.0..=1.0,
            cfg.mid,
            move |v| Message::ToneStackMidChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
        labeled_slider(
            "Treble",
            0.0..=1.0,
            cfg.treble,
            move |v| Message::ToneStackTrebleChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
        labeled_slider(
            "Presence",
            0.0..=1.0,
            cfg.presence,
            move |v| Message::ToneStackPresenceChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
    ]
    .spacing(5);

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
