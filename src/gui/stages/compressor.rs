use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::compressor::CompressorStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_card};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CompressorConfig {
    pub attack_ms: f32,
    pub release_ms: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub makeup_db: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            attack_ms: 1.0,
            release_ms: 100.0,
            threshold_db: -20.0,
            ratio: 4.0,
            makeup_db: 0.0,
        }
    }
}

impl CompressorConfig {
    pub fn to_stage(&self, sample_rate: f32) -> CompressorStage {
        CompressorStage::new(
            self.attack_ms,
            self.release_ms,
            self.threshold_db,
            self.ratio,
            self.makeup_db,
            sample_rate,
        )
    }

    pub const fn apply(&mut self, msg: CompressorMessage) {
        match msg {
            CompressorMessage::ThresholdChanged(v) => self.threshold_db = v,
            CompressorMessage::RatioChanged(v) => self.ratio = v,
            CompressorMessage::AttackChanged(v) => self.attack_ms = v,
            CompressorMessage::ReleaseChanged(v) => self.release_ms = v,
            CompressorMessage::MakeupChanged(v) => self.makeup_db = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum CompressorMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    ReleaseChanged(f32),
    MakeupChanged(f32),
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &CompressorConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_compressor),
        idx,
        total_stages,
        is_collapsed,
        || {
            column![
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
            .spacing(5)
            .into()
        },
    )
}
