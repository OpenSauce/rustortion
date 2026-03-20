use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::tonestack::{ToneStackConfig, ToneStackModel};
use crate::gui::components::widgets::common::{
    labeled_picker, labeled_slider, stage_card, StageViewState, SPACING_TIGHT,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum ToneStackMessage {
    ModelChanged(ToneStackModel),
    BassChanged(f32),
    MidChanged(f32),
    TrebleChanged(f32),
    PresenceChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut ToneStackConfig, msg: ToneStackMessage) -> Option<ParamUpdate> {
    match msg {
        ToneStackMessage::ModelChanged(mo) => { cfg.model = mo; Some(ParamUpdate::NeedsStageRebuild) }
        ToneStackMessage::BassChanged(v) => { cfg.bass = v; Some(ParamUpdate::Changed("bass", v)) }
        ToneStackMessage::MidChanged(v) => { cfg.mid = v; Some(ParamUpdate::Changed("mid", v)) }
        ToneStackMessage::TrebleChanged(v) => { cfg.treble = v; Some(ParamUpdate::Changed("treble", v)) }
        ToneStackMessage::PresenceChanged(v) => { cfg.presence = v; Some(ParamUpdate::Changed("presence", v)) }
    }
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
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_tone_stack),
        idx,
        state,
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
