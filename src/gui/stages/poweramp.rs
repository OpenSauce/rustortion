use iced::widget::{column, pick_list, row, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::poweramp::{PowerAmpStage, PowerAmpType};
use crate::gui::components::widgets::common::{labeled_slider, stage_card};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PowerAmpConfig {
    pub drive: f32,
    pub amp_type: PowerAmpType,
    pub sag: f32,
}

impl Default for PowerAmpConfig {
    fn default() -> Self {
        Self {
            drive: 0.5,
            amp_type: PowerAmpType::ClassAB,
            sag: 0.3,
        }
    }
}

impl PowerAmpConfig {
    pub fn to_stage(&self, sample_rate: f32) -> PowerAmpStage {
        PowerAmpStage::new(self.drive, self.amp_type, self.sag, sample_rate)
    }

    pub fn apply(&mut self, msg: PowerAmpMessage) {
        match msg {
            PowerAmpMessage::TypeChanged(t) => self.amp_type = t,
            PowerAmpMessage::DriveChanged(v) => self.drive = v,
            PowerAmpMessage::SagChanged(v) => self.sag = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum PowerAmpMessage {
    TypeChanged(PowerAmpType),
    DriveChanged(f32),
    SagChanged(f32),
}

// --- View ---

const POWER_AMP_TYPES: [PowerAmpType; 3] = [
    PowerAmpType::ClassA,
    PowerAmpType::ClassAB,
    PowerAmpType::ClassB,
];

pub fn view(
    idx: usize,
    cfg: &PowerAmpConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_power_amp),
        idx,
        total_stages,
        is_collapsed,
        || {
            let type_picker = row![
                text(tr!(type_label)).width(Length::FillPortion(3)),
                pick_list(POWER_AMP_TYPES, Some(cfg.amp_type), move |t| {
                    Message::Stage(idx, StageMessage::PowerAmp(PowerAmpMessage::TypeChanged(t)))
                })
                .width(Length::FillPortion(7)),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            column![
                type_picker,
                labeled_slider(
                    tr!(drive),
                    0.0..=1.0,
                    cfg.drive,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::PowerAmp(PowerAmpMessage::DriveChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
                labeled_slider(
                    tr!(sag),
                    0.0..=1.0,
                    cfg.sag,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::PowerAmp(PowerAmpMessage::SagChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
            ]
            .spacing(5)
            .into()
        },
    )
}
