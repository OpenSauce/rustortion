use iced::widget::{column, container};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::noise_gate::NoiseGateStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NoiseGateConfig {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
}

impl Default for NoiseGateConfig {
    fn default() -> Self {
        Self {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 10.0,
            release_ms: 100.0,
        }
    }
}

impl NoiseGateConfig {
    pub fn to_stage(&self, sample_rate: f32) -> NoiseGateStage {
        NoiseGateStage::new(
            self.threshold_db,
            self.ratio,
            self.attack_ms,
            self.hold_ms,
            self.release_ms,
            sample_rate,
        )
    }

    pub fn apply(&mut self, msg: NoiseGateMessage) {
        match msg {
            NoiseGateMessage::ThresholdChanged(v) => self.threshold_db = v,
            NoiseGateMessage::RatioChanged(v) => self.ratio = v,
            NoiseGateMessage::AttackChanged(v) => self.attack_ms = v,
            NoiseGateMessage::HoldChanged(v) => self.hold_ms = v,
            NoiseGateMessage::ReleaseChanged(v) => self.release_ms = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum NoiseGateMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    HoldChanged(f32),
    ReleaseChanged(f32),
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &NoiseGateConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_noise_gate), idx, total_stages, is_collapsed);

    let mut content = column![header].spacing(5);

    if !is_collapsed {
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

        content = content.push(body);
    }

    let padding = if is_collapsed { 5 } else { 10 };

    container(content.padding(padding))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
