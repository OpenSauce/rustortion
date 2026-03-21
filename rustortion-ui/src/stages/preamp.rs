use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::clipper::ClipperType;
use rustortion_core::amp::stages::preamp::PreampConfig;
use crate::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, StageViewState, SPACING_TIGHT,
};
use crate::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum PreampMessage {
    GainChanged(f32),
    BiasChanged(f32),
    ClipperChanged(ClipperType),
}

// --- Apply ---

pub const fn apply(cfg: &mut PreampConfig, msg: PreampMessage) -> Option<ParamUpdate> {
    match msg {
        PreampMessage::GainChanged(v) => { cfg.gain = v; Some(ParamUpdate::Changed("gain", v)) }
        PreampMessage::BiasChanged(v) => { cfg.bias = v; Some(ParamUpdate::Changed("bias", v)) }
        PreampMessage::ClipperChanged(c) => { cfg.clipper_type = c; Some(ParamUpdate::NeedsStageRebuild) }
    }
}

// --- View ---

const CLIPPER_TYPES: [ClipperType; 6] = [
    ClipperType::Soft,
    ClipperType::Medium,
    ClipperType::Hard,
    ClipperType::Asymmetric,
    ClipperType::ClassA,
    ClipperType::Triode,
];

pub fn view(
    idx: usize,
    cfg: &PreampConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(tr!(stage_preamp), idx, state, || {
        column![
            labeled_picker(tr!(clipper), CLIPPER_TYPES, Some(cfg.clipper_type), move |t| {
                Message::Stage(idx, StageMessage::Preamp(PreampMessage::ClipperChanged(t)))
            }),
            labeled_slider(
                tr!(gain),
                0.0..=10.0,
                cfg.gain,
                move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::GainChanged(v))),
                |v| format!("{v:.1}"),
                0.1
            ),
            labeled_slider(
                tr!(bias),
                -1.0..=1.0,
                cfg.bias,
                move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::BiasChanged(v))),
                |v| format!("{v:.2}"),
                0.1
            ),
        ]
        .spacing(SPACING_TIGHT)
        .into()
    })
}
