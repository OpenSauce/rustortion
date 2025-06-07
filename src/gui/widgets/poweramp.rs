use super::{icon_button, labeled_slider};
use crate::gui::amp::{Message, PowerAmpConfig};
use crate::sim::stages::poweramp::PowerAmpType;
use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

const POWER_AMP_TYPES: [PowerAmpType; 3] = [
    PowerAmpType::ClassA,
    PowerAmpType::ClassAB,
    PowerAmpType::ClassB,
];

pub fn poweramp_widget(idx: usize, cfg: &PowerAmpConfig, total_stages: usize) -> Element<Message> {
    let mut header = row![text(format!("Power Amp {}", idx + 1))].spacing(5);

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

    let type_picker = row![
        text("Type:").width(Length::FillPortion(3)),
        pick_list(POWER_AMP_TYPES, Some(cfg.amp_type), move |t| {
            Message::PowerAmpTypeChanged(idx, t)
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        type_picker,
        labeled_slider(
            "Drive",
            0.0..=1.0,
            cfg.drive,
            move |v| Message::PowerAmpDriveChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
        labeled_slider(
            "Sag",
            0.0..=1.0,
            cfg.sag,
            move |v| Message::PowerAmpSagChanged(idx, v),
            |v| format!("{:.2}", v),
            0.1
        ),
    ]
    .spacing(5);

    container(
        column![header.align_y(iced::Alignment::Center), body]
            .spacing(5)
            .padding(10),
    )
    .width(Length::Fill)
    .style(|theme: &iced::Theme| {
        container::Style::default()
            .background(theme.palette().background)
            .border(iced::Border::default().rounded(5))
    })
    .into()
}
