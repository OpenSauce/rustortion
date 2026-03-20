use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::poweramp::{PowerAmpConfig, PowerAmpType};
use crate::gui::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, StageViewState, SPACING_TIGHT,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum PowerAmpMessage {
    TypeChanged(PowerAmpType),
    DriveChanged(f32),
    SagChanged(f32),
    SagReleaseChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut PowerAmpConfig, msg: PowerAmpMessage) -> Option<ParamUpdate> {
    match msg {
        PowerAmpMessage::TypeChanged(t) => { cfg.amp_type = t; Some(ParamUpdate::NeedsStageRebuild) }
        PowerAmpMessage::DriveChanged(v) => { cfg.drive = v; Some(ParamUpdate::Changed("drive", v)) }
        PowerAmpMessage::SagChanged(v) => { cfg.sag = v; Some(ParamUpdate::Changed("sag", v)) }
        PowerAmpMessage::SagReleaseChanged(v) => { cfg.sag_release = v; Some(ParamUpdate::Changed("sag_release", v)) }
    }
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
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_power_amp),
        idx,
        state,
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
    use rustortion_core::amp::stages::poweramp::PowerAmpConfig;

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
