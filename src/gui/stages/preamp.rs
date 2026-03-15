use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::clipper::ClipperType;
use crate::amp::stages::preamp::PreampStage;
use crate::gui::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, StageViewState, SPACING_TIGHT,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PreampConfig {
    pub gain: f32,
    pub bias: f32,
    pub clipper_type: ClipperType,
    #[serde(default)]
    pub bypassed: bool,
}

impl Default for PreampConfig {
    fn default() -> Self {
        Self {
            gain: 5.0,
            bias: 0.0,
            clipper_type: ClipperType::Soft,
            bypassed: false,
        }
    }
}

impl PreampConfig {
    pub fn to_stage(&self, sample_rate: f32) -> PreampStage {
        PreampStage::new(self.gain, self.bias, self.clipper_type, sample_rate)
    }

    pub const fn apply(&mut self, msg: PreampMessage) -> Option<ParamUpdate> {
        match msg {
            PreampMessage::GainChanged(v) => { self.gain = v; Some(ParamUpdate::Changed("gain", v)) }
            PreampMessage::BiasChanged(v) => { self.bias = v; Some(ParamUpdate::Changed("bias", v)) }
            PreampMessage::ClipperChanged(c) => { self.clipper_type = c; Some(ParamUpdate::NeedsStageRebuild) }
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum PreampMessage {
    GainChanged(f32),
    BiasChanged(f32),
    ClipperChanged(ClipperType),
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
