use super::labeled_slider;
use crate::gui::amp::{Message, PreampConfig};
use crate::sim::stages::clipper::ClipperType;
use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

const CLIPPER_TYPES: [ClipperType; 5] = [
    ClipperType::Soft,
    ClipperType::Medium,
    ClipperType::Hard,
    ClipperType::Asymmetric,
    ClipperType::ClassA,
];

pub fn preamp_widget(idx: usize, cfg: &PreampConfig) -> Element<Message> {
    let header = row![
        text(format!("Preamp {}", idx + 1)),
        iced::widget::button("x").on_press(Message::RemoveStage(idx)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let clipper_picker = row![
        text("Clipper:").width(Length::FillPortion(3)),
        pick_list(CLIPPER_TYPES, Some(cfg.clipper_type), move |t| {
            Message::PreampClipperChanged(idx, t)
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        clipper_picker,
        labeled_slider(
            "Gain",
            0.0..=10.0,
            cfg.gain,
            move |v| Message::PreampGainChanged(idx, v),
            |v| format!("{:.1}", v),
            1.0
        ),
        labeled_slider(
            "Bias",
            -1.0..=1.0,
            cfg.bias,
            move |v| Message::PreampBiasChanged(idx, v),
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
