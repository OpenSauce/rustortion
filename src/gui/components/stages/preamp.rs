use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

use crate::amp::stages::clipper::ClipperType;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::PreampConfig;
use crate::gui::messages::{Message, PreampMessage, StageMessage};
use crate::tr;

const CLIPPER_TYPES: [ClipperType; 5] = [
    ClipperType::Soft,
    ClipperType::Medium,
    ClipperType::Hard,
    ClipperType::Asymmetric,
    ClipperType::ClassA,
];

pub fn view(idx: usize, cfg: &PreampConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_preamp), idx, total_stages);

    let clipper_picker = row![
        text(tr!(clipper)).width(Length::FillPortion(3)),
        pick_list(CLIPPER_TYPES, Some(cfg.clipper_type), move |t| {
            Message::Stage(idx, StageMessage::Preamp(PreampMessage::ClipperChanged(t)))
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        clipper_picker,
        labeled_slider(
            tr!(gain),
            0.0..=10.0,
            cfg.gain,
            move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::GainChanged(v))),
            |v| format!("{v:.1}"),
            0.1
        ),
        labeled_slider(
            tr!(bias),
            -1.0..=1.0,
            cfg.bias,
            move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::BiasChanged(v))),
            |v| format!("{v:.2}"),
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
