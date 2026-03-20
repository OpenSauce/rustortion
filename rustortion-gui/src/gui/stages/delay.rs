use iced::widget::column;
use iced::Element;

use rustortion_core::amp::stages::delay::DelayConfig;
use crate::gui::components::widgets::common::{labeled_slider, stage_card, StageViewState, SPACING_TIGHT};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum DelayMessage {
    DelayTimeChanged(f32),
    FeedbackChanged(f32),
    MixChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut DelayConfig, msg: DelayMessage) -> Option<ParamUpdate> {
    match msg {
        DelayMessage::DelayTimeChanged(v) => { cfg.delay_ms = v; Some(ParamUpdate::Changed("delay_time", v)) }
        DelayMessage::FeedbackChanged(v) => { cfg.feedback = v; Some(ParamUpdate::Changed("feedback", v)) }
        DelayMessage::MixChanged(v) => { cfg.mix = v; Some(ParamUpdate::Changed("mix", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &DelayConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_delay),
        idx,
        state,
        || {
            column![
                labeled_slider(
                    tr!(delay_time),
                    0.0..=2000.0,
                    cfg.delay_ms,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::DelayTimeChanged(v))
                    ),
                    |v| format!("{v:.0} {}", tr!(ms)),
                    1.0
                ),
                labeled_slider(
                    tr!(feedback),
                    0.0..=0.95,
                    cfg.feedback,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::FeedbackChanged(v))
                    ),
                    |v| format!("{v:.2}"),
                    0.01
                ),
                labeled_slider(
                    tr!(dry_wet),
                    0.0..=1.0,
                    cfg.mix,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::Delay(DelayMessage::MixChanged(v))
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
