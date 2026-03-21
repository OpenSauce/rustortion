use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::noise_gate::NoiseGateConfig;
use crate::components::widgets::common::{labeled_slider, stage_card, StageViewState, SPACING_TIGHT};
use crate::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum NoiseGateMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    HoldChanged(f32),
    ReleaseChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut NoiseGateConfig, msg: NoiseGateMessage) -> Option<ParamUpdate> {
    match msg {
        NoiseGateMessage::ThresholdChanged(v) => { cfg.threshold_db = v; Some(ParamUpdate::Changed("threshold", v)) }
        NoiseGateMessage::RatioChanged(v) => { cfg.ratio = v; Some(ParamUpdate::Changed("ratio", v)) }
        NoiseGateMessage::AttackChanged(v) => { cfg.attack_ms = v; Some(ParamUpdate::Changed("attack", v)) }
        NoiseGateMessage::HoldChanged(v) => { cfg.hold_ms = v; Some(ParamUpdate::Changed("hold", v)) }
        NoiseGateMessage::ReleaseChanged(v) => { cfg.release_ms = v; Some(ParamUpdate::Changed("release", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &NoiseGateConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_noise_gate),
        idx,
        state,
        || {
            column![
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
            .spacing(SPACING_TIGHT)
            .into()
        },
    )
}
