use iced::widget::column;
use iced::Element;
use serde::{Deserialize, Serialize};

use crate::amp::stages::tonestack::{ToneStackModel, ToneStackStage};
use crate::gui::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, SPACING_TIGHT,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ToneStackConfig {
    pub model: ToneStackModel,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    #[serde(default)]
    pub bypassed: bool,
}

impl Default for ToneStackConfig {
    fn default() -> Self {
        Self {
            model: ToneStackModel::Modern,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            bypassed: false,
        }
    }
}

impl ToneStackConfig {
    pub const fn to_stage(&self, sample_rate: f32) -> ToneStackStage {
        ToneStackStage::new(
            self.model,
            self.bass,
            self.mid,
            self.treble,
            self.presence,
            sample_rate,
        )
    }

    pub const fn apply(&mut self, msg: ToneStackMessage) -> Option<ParamUpdate> {
        match msg {
            ToneStackMessage::ModelChanged(mo) => { self.model = mo; Some(ParamUpdate::NeedsStageRebuild) }
            ToneStackMessage::BassChanged(v) => { self.bass = v; Some(ParamUpdate::Changed("bass", v)) }
            ToneStackMessage::MidChanged(v) => { self.mid = v; Some(ParamUpdate::Changed("mid", v)) }
            ToneStackMessage::TrebleChanged(v) => { self.treble = v; Some(ParamUpdate::Changed("treble", v)) }
            ToneStackMessage::PresenceChanged(v) => { self.presence = v; Some(ParamUpdate::Changed("presence", v)) }
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum ToneStackMessage {
    ModelChanged(ToneStackModel),
    BassChanged(f32),
    MidChanged(f32),
    TrebleChanged(f32),
    PresenceChanged(f32),
}

// --- View ---

const TONE_STACK_MODELS: [ToneStackModel; 4] = [
    ToneStackModel::Modern,
    ToneStackModel::British,
    ToneStackModel::American,
    ToneStackModel::Flat,
];

pub fn view(
    idx: usize,
    cfg: &ToneStackConfig,
    is_collapsed: bool,
    can_move_up: bool,
    can_move_down: bool,
    bypassed: bool,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_tone_stack),
        idx,
        is_collapsed,
        can_move_up,
        can_move_down,
        bypassed,
        || {
            column![
                labeled_picker(tr!(model), TONE_STACK_MODELS, Some(cfg.model), move |m| {
                    Message::Stage(
                        idx,
                        StageMessage::ToneStack(ToneStackMessage::ModelChanged(m)),
                    )
                }),
                labeled_slider(
                    tr!(bass),
                    0.0..=2.0,
                    cfg.bass,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::ToneStack(ToneStackMessage::BassChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
                labeled_slider(
                    tr!(mid),
                    0.0..=2.0,
                    cfg.mid,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::ToneStack(ToneStackMessage::MidChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
                labeled_slider(
                    tr!(treble),
                    0.0..=2.0,
                    cfg.treble,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::ToneStack(ToneStackMessage::TrebleChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
                labeled_slider(
                    tr!(presence),
                    0.0..=2.0,
                    cfg.presence,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::ToneStack(ToneStackMessage::PresenceChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.05
                ),
            ]
            .spacing(SPACING_TIGHT)
            .into()
        },
    )
}
