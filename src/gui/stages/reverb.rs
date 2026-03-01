use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::reverb::ReverbStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_card};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReverbConfig {
    pub room_size: f32,
    pub damping: f32,
    pub mix: f32,
}

impl Default for ReverbConfig {
    fn default() -> Self {
        Self {
            room_size: 0.5,
            damping: 0.5,
            mix: 0.2,
        }
    }
}

impl ReverbConfig {
    pub fn to_stage(&self, sample_rate: f32) -> ReverbStage {
        ReverbStage::new(self.room_size, self.damping, self.mix, sample_rate)
    }

    pub const fn apply(&mut self, msg: ReverbMessage) {
        match msg {
            ReverbMessage::RoomSizeChanged(v) => self.room_size = v,
            ReverbMessage::DampingChanged(v) => self.damping = v,
            ReverbMessage::MixChanged(v) => self.mix = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum ReverbMessage {
    RoomSizeChanged(f32),
    DampingChanged(f32),
    MixChanged(f32),
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &ReverbConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_reverb),
        idx,
        total_stages,
        is_collapsed,
        || {
            column![
                labeled_slider(
                    tr!(room_size),
                    0.0..=1.0,
                    cfg.room_size,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::RoomSizeChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(damping),
                    0.0..=1.0,
                    cfg.damping,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::DampingChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(dry_wet),
                    0.0..=1.0,
                    cfg.mix,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::MixChanged(v))
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
