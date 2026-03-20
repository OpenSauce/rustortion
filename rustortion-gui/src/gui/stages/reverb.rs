use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::reverb::ReverbConfig;
use crate::gui::components::widgets::common::{labeled_slider, stage_card, StageViewState, SPACING_TIGHT};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum ReverbMessage {
    RoomSizeChanged(f32),
    DampingChanged(f32),
    MixChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut ReverbConfig, msg: ReverbMessage) -> Option<ParamUpdate> {
    match msg {
        ReverbMessage::RoomSizeChanged(v) => { cfg.room_size = v; Some(ParamUpdate::Changed("room_size", v)) }
        ReverbMessage::DampingChanged(v) => { cfg.damping = v; Some(ParamUpdate::Changed("damping", v)) }
        ReverbMessage::MixChanged(v) => { cfg.mix = v; Some(ParamUpdate::Changed("mix", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &ReverbConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_reverb),
        idx,
        state,
        || {
            column![
                labeled_slider(
                    tr!(room_size),
                    0.0..=1.0,
                    cfg.room_size,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::RoomSizeChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(damping),
                    0.0..=1.0,
                    cfg.damping,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::DampingChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(dry_wet),
                    0.0..=1.0,
                    cfg.mix,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Reverb(ReverbMessage::MixChanged(v))
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
            ]
            .spacing(SPACING_TIGHT)
            .into()
        },
    )
}
