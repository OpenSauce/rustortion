use super::labeled_slider;
use crate::gui::amp::{Message, PowerAmpConfig};
use crate::sim::stages::poweramp::PowerAmpType;
use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

const POWER_AMP_TYPES: [PowerAmpType; 3] = [
    PowerAmpType::ClassA,
    PowerAmpType::ClassAB,
    PowerAmpType::ClassB,
];

pub fn poweramp_widget(idx: usize, cfg: &PowerAmpConfig) -> Element<Message> {
    let header = row![
        text(format!("Power Amp {}", idx + 1)),
        iced::widget::button("x").on_press(Message::RemoveStage(idx)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

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

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
