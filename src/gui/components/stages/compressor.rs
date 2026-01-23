use iced::widget::{column, container};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::CompressorConfig;
use crate::gui::messages::{CompressorMessage, Message, StageMessage};
use crate::tr;

pub fn view(idx: usize, cfg: &CompressorConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_compressor), idx, total_stages);

    let body = column![
        labeled_slider(
            tr!(threshold),
            -60.0..=0.0,
            cfg.threshold_db,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::ThresholdChanged(v))
            ),
            |v| format!("{v:.1} {}", tr!(db)),
            1.0
        ),
        labeled_slider(
            tr!(ratio),
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
            tr!(attack),
            0.1..=100.0,
            cfg.attack_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::AttackChanged(v))
            ),
            |v| format!("{v:.1} {}", tr!(ms)),
            0.1
        ),
        labeled_slider(
            tr!(release),
            10.0..=1000.0,
            cfg.release_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::ReleaseChanged(v))
            ),
            |v| format!("{v:.0} {}", tr!(ms)),
            1.0
        ),
        labeled_slider(
            tr!(makeup),
            -12.0..=24.0,
            cfg.makeup_db,
            move |v| Message::Stage(
                idx,
                StageMessage::Compressor(CompressorMessage::MakeupChanged(v))
            ),
            |v| format!("{v:.1} {}", tr!(db)),
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
