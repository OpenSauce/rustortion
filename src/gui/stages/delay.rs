use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::delay::DelayStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_card};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DelayConfig {
    pub delay_ms: f32,
    pub feedback: f32,
    pub mix: f32,
}

impl Default for DelayConfig {
    fn default() -> Self {
        Self {
            delay_ms: 300.0,
            feedback: 0.3,
            mix: 0.5,
        }
    }
}

impl DelayConfig {
    pub fn to_stage(&self, sample_rate: f32) -> DelayStage {
        DelayStage::new(self.delay_ms, self.feedback, self.mix, sample_rate)
    }

    pub const fn apply(&mut self, msg: DelayMessage) {
        match msg {
            DelayMessage::DelayTimeChanged(v) => self.delay_ms = v,
            DelayMessage::FeedbackChanged(v) => self.feedback = v,
            DelayMessage::MixChanged(v) => self.mix = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum DelayMessage {
    DelayTimeChanged(f32),
    FeedbackChanged(f32),
    MixChanged(f32),
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &DelayConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_delay),
        idx,
        total_stages,
        is_collapsed,
        || {
            column![
                labeled_slider(
                    tr!(delay_time),
                    0.0..=2000.0,
                    cfg.delay_ms,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::DelayTimeChanged(v))
                    ),
                    |v| format!("{v:.0} {}", tr!(ms)),
                    1.0
                ),
                labeled_slider(
                    tr!(feedback),
                    0.0..=0.95,
                    cfg.feedback,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::FeedbackChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.01
                ),
                labeled_slider(
                    tr!(dry_wet),
                    0.0..=1.0,
                    cfg.mix,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::MixChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
            ]
            .spacing(5)
            .into()
        },
    )
}
