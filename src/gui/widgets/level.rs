use super::{labeled_slider, stage_header};
use crate::gui::amp::{LevelConfig, Message};
use iced::widget::{column, container};
use iced::{Element, Length};

const HEADER_TEXT: &str = "Level";

pub fn level_widget(idx: usize, cfg: &LevelConfig, total_stages: usize) -> Element<Message> {
    let header = stage_header(HEADER_TEXT, idx, total_stages);

    let body = column![labeled_slider(
        "Gain",
        0.0..=2.0,
        cfg.gain,
        move |v| Message::LevelGainChanged(idx, v),
        |v| format!("{v:.2}"),
        0.05
    ),]
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
