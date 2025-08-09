use iced::widget::{column, container};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::CompressorConfig;
use crate::gui::messages::{CompressorMessage, Message, StageMessage};

const HEADER_TEXT: &str = "Compressor";

pub fn view(idx: usize, cfg: &CompressorConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(HEADER_TEXT, idx, total_stages);

    let body = column![
        labeled_slider(
            "Threshold",
            -60.0..=0.0,
            cfg.threshold_db,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::ThresholdChanged(v))
            ),
            |v| format!("{v:.1} dB"),
            1.0
        ),
        labeled_slider(
            "Ratio",
            1.0..=20.0,
            cfg.ratio,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::RatioChanged(v))
            ),
            |v| format!("{v:.1}:1"),
            0.1
        ),
        labeled_slider(
            "Attack",
            0.1..=100.0,
            cfg.attack_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::AttackChanged(v))
            ),
            |v| format!("{v:.1} ms"),
            0.1
        ),
        labeled_slider(
            "Release",
            10.0..=1000.0,
            cfg.release_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::ReleaseChanged(v))
            ),
            |v| format!("{v:.0} ms"),
            1.0
        ),
        labeled_slider(
            "Makeup",
            -12.0..=24.0,
            cfg.makeup_db,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::MakeupChanged(v))
            ),
            |v| format!("{v:.1} dB"),
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
