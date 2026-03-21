use iced::widget::row;
use iced::{Element, Length};

use rustortion_core::amp::stages::eq::{BAND_FREQS, EqConfig, MAX_GAIN_DB, MIN_GAIN_DB, NUM_BANDS};
use crate::components::widgets::common::{
    labeled_vertical_slider, stage_card, StageViewState, SPACING_WIDE,
};
use crate::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

const BAND_NAMES: [&str; 16] = [
    "band_0", "band_1", "band_2", "band_3",
    "band_4", "band_5", "band_6", "band_7",
    "band_8", "band_9", "band_10", "band_11",
    "band_12", "band_13", "band_14", "band_15",
];

// --- Message ---

#[derive(Debug, Clone, Copy)]
pub enum EqMessage {
    GainChanged(usize, f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut EqConfig, msg: EqMessage) -> Option<ParamUpdate> {
    match msg {
        EqMessage::GainChanged(band, value) => {
            if band < NUM_BANDS {
                let clamped = if value < MIN_GAIN_DB {
                    MIN_GAIN_DB
                } else if value > MAX_GAIN_DB {
                    MAX_GAIN_DB
                } else {
                    value
                };
                cfg.gains[band] = clamped;
                Some(ParamUpdate::Changed(BAND_NAMES[band], clamped))
            } else {
                None
            }
        }
    }
}

// --- Helpers ---

fn format_freq(hz: f64) -> String {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        if (k - k.round()).abs() < 0.01 {
            format!("{}k", k as u32)
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{}", hz as u32)
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &EqConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_eq),
        idx,
        state,
        || {
            let mut faders = row![].spacing(SPACING_WIDE);
            for (band, &freq) in BAND_FREQS.iter().enumerate() {
                faders = faders.push(labeled_vertical_slider(
                    format_freq(freq),
                    MIN_GAIN_DB..=MAX_GAIN_DB,
                    cfg.gains[band],
                    move |v| {
                        Message::Stage(idx, StageMessage::Eq(EqMessage::GainChanged(band, v)))
                    },
                    |v| format!("{v:+.1}"),
                    0.1,
                    150.0,
                ));
            }
            iced::widget::container(faders)
                .width(Length::Fill)
                .center_x(Length::Fill)
                .into()
        },
    )
}
