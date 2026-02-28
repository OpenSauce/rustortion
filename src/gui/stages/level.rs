use iced::widget::{column, container};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::level::LevelStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LevelConfig {
    pub gain: f32,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl LevelConfig {
    pub fn to_stage(&self) -> LevelStage {
        LevelStage::new(self.gain)
    }

    pub fn apply(&mut self, msg: LevelMessage) {
        match msg {
            LevelMessage::GainChanged(v) => self.gain = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum LevelMessage {
    GainChanged(f32),
}

// --- View ---

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
