use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::compressor::CompressorConfig;
use crate::components::widgets::common::{labeled_slider, stage_card, StageViewState, SPACING_TIGHT};
use crate::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum CompressorMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    ReleaseChanged(f32),
    MakeupChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut CompressorConfig, msg: CompressorMessage) -> Option<ParamUpdate> {
    match msg {
        CompressorMessage::ThresholdChanged(v) => { cfg.threshold_db = v; Some(ParamUpdate::Changed("threshold", v)) }
        CompressorMessage::RatioChanged(v) => { cfg.ratio = v; Some(ParamUpdate::Changed("ratio", v)) }
        CompressorMessage::AttackChanged(v) => { cfg.attack_ms = v; Some(ParamUpdate::Changed("attack", v)) }
        CompressorMessage::ReleaseChanged(v) => { cfg.release_ms = v; Some(ParamUpdate::Changed("release", v)) }
        CompressorMessage::MakeupChanged(v) => { cfg.makeup_db = v; Some(ParamUpdate::Changed("makeup", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &CompressorConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_compressor),
        idx,
        state,
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
            .spacing(SPACING_TIGHT)
            .into()
        },
    )
}
