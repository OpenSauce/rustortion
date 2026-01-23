use iced::widget::{column, container};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::NoiseGateConfig;
use crate::gui::messages::{Message, NoiseGateMessage, StageMessage};
use crate::tr;

pub fn view(idx: usize, cfg: &NoiseGateConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_noise_gate), idx, total_stages);

    let body = column![
        labeled_slider(
            tr!(threshold),
            -80.0..=0.0,
            cfg.threshold_db,
            move |v| Message::Stage(
                idx,
                StageMessage::NoiseGate(NoiseGateMessage::ThresholdChanged(v))
            ),
            |v| format!("{v:.1} {}", tr!(db)),
            1.0
        ),
        labeled_slider(
            tr!(ratio),
            1.0..=100.0,
            cfg.ratio,
            move |v| Message::Stage(
                idx,
                StageMessage::NoiseGate(NoiseGateMessage::RatioChanged(v))
            ),
            |v| format!("{v:.0}:1"),
            1.0
        ),
        labeled_slider(
            tr!(attack),
            0.1..=100.0,
            cfg.attack_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::NoiseGate(NoiseGateMessage::AttackChanged(v))
            ),
            |v| format!("{v:.1} {}", tr!(ms)),
            0.1
        ),
        labeled_slider(
            tr!(hold),
            0.0..=500.0,
            cfg.hold_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::NoiseGate(NoiseGateMessage::HoldChanged(v))
            ),
            |v| format!("{v:.0} {}", tr!(ms)),
            1.0
        ),
        labeled_slider(
            tr!(release),
            1.0..=1000.0,
            cfg.release_ms,
            move |v| Message::Stage(
                idx,
                StageMessage::NoiseGate(NoiseGateMessage::ReleaseChanged(v))
            ),
            |v| format!("{v:.0} {}", tr!(ms)),
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
