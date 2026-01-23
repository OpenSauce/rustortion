use iced::widget::{column, container};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::LevelConfig;
use crate::gui::messages::{LevelMessage, Message, StageMessage};
use crate::tr;

pub fn view(idx: usize, cfg: &LevelConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_level), idx, total_stages);

    let body = column![labeled_slider(
        tr!(gain),
        0.0..=2.0,
        cfg.gain,
        move |v| Message::Stage(idx, StageMessage::Level(LevelMessage::GainChanged(v))),
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
