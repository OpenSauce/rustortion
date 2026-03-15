use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::poweramp::{PowerAmpStage, PowerAmpType};
use crate::gui::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, SPACING_TIGHT,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct PowerAmpConfig {
    pub drive: f32,
    pub amp_type: PowerAmpType,
    pub sag: f32,
    pub sag_release: f32,
}

impl Default for PowerAmpConfig {
    fn default() -> Self {
        Self {
            drive: 0.5,
            amp_type: PowerAmpType::ClassAB,
            sag: 0.3,
            sag_release: 120.0,
        }
    }
}

impl PowerAmpConfig {
    pub fn to_stage(&self, sample_rate: f32) -> PowerAmpStage {
        PowerAmpStage::new(self.drive, self.amp_type, self.sag, self.sag_release, sample_rate)
    }

    pub const fn apply(&mut self, msg: PowerAmpMessage) -> Option<ParamUpdate> {
        match msg {
            PowerAmpMessage::TypeChanged(t) => { self.amp_type = t; Some(ParamUpdate::NeedsStageRebuild) }
            PowerAmpMessage::DriveChanged(v) => { self.drive = v; Some(ParamUpdate::Changed("drive", v)) }
            PowerAmpMessage::SagChanged(v) => { self.sag = v; Some(ParamUpdate::Changed("sag", v)) }
            PowerAmpMessage::SagReleaseChanged(v) => { self.sag_release = v; Some(ParamUpdate::Changed("sag_release", v)) }
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum PowerAmpMessage {
    TypeChanged(PowerAmpType),
    DriveChanged(f32),
    SagChanged(f32),
    SagReleaseChanged(f32),
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
    is_collapsed: bool,
    can_move_up: bool,
    can_move_down: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_power_amp),
        idx,
        is_collapsed,
        can_move_up,
        can_move_down,
        || {
            column![
                labeled_picker(tr!(type_label), POWER_AMP_TYPES, Some(cfg.amp_type), move |t| {
                    Message::Stage(idx, StageMessage::PowerAmp(PowerAmpMessage::TypeChanged(t)))
                }),
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
                labeled_slider(
                    tr!(sag_release),
                    40.0..=200.0,
                    cfg.sag_release,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::PowerAmp(PowerAmpMessage::SagReleaseChanged(v))
                    ),
                    |v| format!("{v:.0} {}", tr!(ms)),
                    5.0
                ),
            ]
            .spacing(SPACING_TIGHT)
            .into()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_backward_compat() {
        let json = r#"{"drive":0.5,"amp_type":"ClassAB","sag":0.3}"#;
        let cfg: PowerAmpConfig = serde_json::from_str(json).unwrap();
        assert!(
            (cfg.sag_release - 120.0).abs() < 1e-6,
            "missing sag_release should default to 120.0, got {}",
            cfg.sag_release
        );
    }
}
