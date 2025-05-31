use super::labeled_slider;
use crate::gui::amp::{Message, ToneStackConfig};
use crate::sim::stages::tonestack::ToneStackModel;
use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

pub fn tonestack_widget(idx: usize, cfg: &ToneStackConfig) -> Element<Message> {
    let header = row![
        text(format!("Tone Stack {}", idx + 1)),
        iced::widget::button("x").on_press(Message::RemoveStage(idx)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let models = vec![
        ToneStackModel::Modern,
        ToneStackModel::British,
        ToneStackModel::American,
        ToneStackModel::Flat,
    ];

    let model_picker = row![
        text("Model:").width(Length::FillPortion(3)),
        pick_list(models, Some(cfg.model), move |m| {
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
            |v| format!("{:.2}", v)
        ),
        labeled_slider(
            "Mid",
            0.0..=1.0,
            cfg.mid,
            move |v| Message::ToneStackMidChanged(idx, v),
            |v| format!("{:.2}", v)
        ),
        labeled_slider(
            "Treble",
            0.0..=1.0,
            cfg.treble,
            move |v| Message::ToneStackTrebleChanged(idx, v),
            |v| format!("{:.2}", v)
        ),
        labeled_slider(
            "Presence",
            0.0..=1.0,
            cfg.presence,
            move |v| Message::ToneStackPresenceChanged(idx, v),
            |v| format!("{:.2}", v)
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
