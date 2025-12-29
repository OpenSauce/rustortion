use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::FilterConfig;
use crate::gui::messages::{FilterMessage, Message, StageMessage};
use crate::sim::stages::filter::FilterType;

const HEADER_TEXT: &str = "Filter";
const FILTER_TYPES: [FilterType; 2] = [FilterType::Highpass, FilterType::Lowpass];

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

    let range = match cfg.filter_type {
        FilterType::Highpass => 0.0..=1000.0,
        FilterType::Lowpass => 5000.0..=15000.0,
    };

    let body = column![
        type_picker,
        labeled_slider(
            "Cutoff",
            range,
            cfg.cutoff_hz,
            move |v| Message::Stage(idx, StageMessage::Filter(FilterMessage::CutoffChanged(v))),
            |v| format!("{v:.0} Hz"),
            1.0
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
