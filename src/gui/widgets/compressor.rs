use super::{icon_button, labeled_slider};
use crate::gui::amp::{CompressorConfig, Message};
use iced::widget::{column, container, row, text};
use iced::{Element, Length};

pub fn compressor_widget(
    idx: usize,
    cfg: &CompressorConfig,
    total_stages: usize,
) -> Element<Message> {
    let mut header = row![text(format!("Compressor {}", idx + 1))].spacing(5);

    if idx > 0 {
        header = header.push(icon_button(
            "↑",
            Some(Message::MoveStageUp(idx)),
            iced::widget::button::primary,
        ));
    } else {
        header = header.push(icon_button("↑", None, iced::widget::button::secondary));
    }

    if idx < total_stages.saturating_sub(1) {
        header = header.push(icon_button(
            "↓",
            Some(Message::MoveStageDown(idx)),
            iced::widget::button::primary,
        ));
    } else {
        header = header.push(icon_button("↓", None, iced::widget::button::secondary));
    }

    header = header.push(icon_button(
        "×",
        Some(Message::RemoveStage(idx)),
        iced::widget::button::danger,
    ));

    let body = column![
        labeled_slider(
            "Threshold",
            -60.0..=0.0,
            cfg.threshold_db,
            move |v| Message::CompressorThresholdChanged(idx, v),
            |v| format!("{:.1} dB", v),
            1.0
        ),
        labeled_slider(
            "Ratio",
            1.0..=20.0,
            cfg.ratio,
            move |v| Message::CompressorRatioChanged(idx, v),
            |v| format!("{:.1}:1", v),
            1.0
        ),
        labeled_slider(
            "Attack",
            0.1..=100.0,
            cfg.attack_ms,
            move |v| Message::CompressorAttackChanged(idx, v),
            |v| format!("{:.1} ms", v),
            1.0
        ),
        labeled_slider(
            "Release",
            10.0..=1000.0,
            cfg.release_ms,
            move |v| Message::CompressorReleaseChanged(idx, v),
            |v| format!("{:.0} ms", v),
            1.0
        ),
        labeled_slider(
            "Makeup",
            -12.0..=24.0,
            cfg.makeup_db,
            move |v| Message::CompressorMakeupChanged(idx, v),
            |v| format!("{:.2} dB", v),
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
