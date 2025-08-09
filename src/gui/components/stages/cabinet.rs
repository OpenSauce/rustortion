use iced::widget::{column, container, text};
use iced::{Element, Length};

use crate::gui::components::widgets::common::stage_header;
use crate::gui::config::CabinetConfig;
use crate::gui::messages::Message;

const HEADER_TEXT: &str = "Cabinet";

pub fn view(idx: usize, _cfg: &CabinetConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(HEADER_TEXT, idx, total_stages);

    let body = text("Cabinet");

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
