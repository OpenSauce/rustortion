use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::FilterConfig;
use crate::gui::messages::{FilterMessage, Message, StageMessage};
use crate::sim::stages::filter::FilterType;

const HEADER_TEXT: &str = "Filter";
const FILTER_TYPES: [FilterType; 4] = [
    FilterType::Highpass,
    FilterType::Lowpass,
    FilterType::Bandpass,
    FilterType::Notch,
];

pub fn view(idx: usize, cfg: &FilterConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(HEADER_TEXT, idx, total_stages);

    let type_picker = row![
        text("Type:").width(Length::FillPortion(3)),
        pick_list(FILTER_TYPES, Some(cfg.filter_type), move |t| {
            Message::Stage(idx, StageMessage::Filter(FilterMessage::TypeChanged(t)))
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        type_picker,
        labeled_slider(
            "Cutoff",
            20.0..=20_000.0,
            cfg.cutoff_hz,
            move |v| Message::Stage(idx, StageMessage::Filter(FilterMessage::CutoffChanged(v))),
            |v| format!("{v:.0} Hz"),
            1.0
        ),
        labeled_slider(
            "Resonance",
            0.0..=1.0,
            cfg.resonance,
            move |v| Message::Stage(
                idx,
                StageMessage::Filter(FilterMessage::ResonanceChanged(v))
            ),
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
