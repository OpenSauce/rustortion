use iced::Element;
use iced::widget::column;

use crate::components::widgets::common::{
    SPACING_TIGHT, StageViewState, labeled_slider, stage_card,
};
use crate::messages::Message;
use crate::tr;
use rustortion_core::amp::stages::tremolo::TremoloConfig;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum TremoloMessage {
    RateChanged(f32),
    DepthChanged(f32),
    ShapeChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut TremoloConfig, msg: TremoloMessage) -> Option<ParamUpdate> {
    match msg {
        TremoloMessage::RateChanged(v) => {
            cfg.rate_hz = v;
            Some(ParamUpdate::Changed("rate", v))
        }
        TremoloMessage::DepthChanged(v) => {
            cfg.depth = v;
            Some(ParamUpdate::Changed("depth", v))
        }
        TremoloMessage::ShapeChanged(v) => {
            cfg.shape = v;
            Some(ParamUpdate::Changed("shape", v))
        }
    }
}

// --- View ---

pub fn view(idx: usize, cfg: &TremoloConfig, state: StageViewState) -> Element<'_, Message> {
    stage_card(tr!(stage_tremolo), idx, state, || {
        column![
            labeled_slider(
                tr!(rate),
                0.1..=20.0,
                cfg.rate_hz,
                move |v| Message::Stage(idx, StageMessage::Tremolo(TremoloMessage::RateChanged(v))),
                |v| format!("{v:.2} {}", tr!(hz)),
                0.01
            ),
            labeled_slider(
                tr!(depth),
                0.0..=1.0,
                cfg.depth,
                move |v| Message::Stage(idx, StageMessage::Tremolo(TremoloMessage::DepthChanged(v))),
                |v| format!("{:.0}%", v * 100.0),
                0.01
            ),
            labeled_slider(
                tr!(shape),
                0.0..=1.0,
                cfg.shape,
                move |v| Message::Stage(idx, StageMessage::Tremolo(TremoloMessage::ShapeChanged(v))),
                |v| format!("{:.0}%", v * 100.0),
                0.01
            ),
        ]
        .spacing(SPACING_TIGHT)
        .into()
    })
}
