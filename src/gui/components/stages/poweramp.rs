use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::PowerAmpConfig;
use crate::gui::messages::{Message, PowerAmpMessage, StageMessage};
use crate::sim::stages::poweramp::PowerAmpType;

const HEADER_TEXT: &str = "Power Amp";
const POWER_AMP_TYPES: [PowerAmpType; 3] = [
    PowerAmpType::ClassA,
    PowerAmpType::ClassAB,
    PowerAmpType::ClassB,
];

pub fn view(idx: usize, cfg: &PowerAmpConfig, total_stages: usize) -> Element<Message> {
    let header = stage_header(HEADER_TEXT, idx, total_stages);

    let type_picker = row![
        text("Type:").width(Length::FillPortion(3)),
        pick_list(POWER_AMP_TYPES, Some(cfg.amp_type), move |t| {
            Message::Stage(idx, StageMessage::PowerAmp(PowerAmpMessage::TypeChanged(t)))
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
            move |v| Message::Stage(
                idx,
                StageMessage::PowerAmp(PowerAmpMessage::DriveChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.05
        ),
        labeled_slider(
            "Sag",
            0.0..=1.0,
            cfg.sag,
            move |v| Message::Stage(idx, StageMessage::PowerAmp(PowerAmpMessage::SagChanged(v))),
            |v| format!("{v:.2}"),
            0.05
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
