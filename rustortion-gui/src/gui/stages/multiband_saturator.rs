use iced::widget::{column, row, text};
use iced::{Element, Length};

use rustortion_core::amp::stages::multiband_saturator::MultibandSaturatorConfig;
use crate::gui::components::widgets::common::{
    SPACING_NORMAL, SPACING_SECTION, SPACING_TIGHT, TEXT_SIZE_INFO, labeled_slider, stage_card,
    StageViewState,
};
use crate::gui::messages::Message;
use crate::tr;

use super::{ParamUpdate, StageMessage};

// --- Message ---

#[derive(Debug, Clone)]
pub enum MultibandSaturatorMessage {
    LowDriveChanged(f32),
    MidDriveChanged(f32),
    HighDriveChanged(f32),
    LowLevelChanged(f32),
    MidLevelChanged(f32),
    HighLevelChanged(f32),
    LowFreqChanged(f32),
    HighFreqChanged(f32),
}

// --- Apply ---

pub const fn apply(cfg: &mut MultibandSaturatorConfig, msg: MultibandSaturatorMessage) -> Option<ParamUpdate> {
    match msg {
        MultibandSaturatorMessage::LowDriveChanged(v) => { cfg.low_drive = v; Some(ParamUpdate::Changed("low_drive", v)) }
        MultibandSaturatorMessage::MidDriveChanged(v) => { cfg.mid_drive = v; Some(ParamUpdate::Changed("mid_drive", v)) }
        MultibandSaturatorMessage::HighDriveChanged(v) => { cfg.high_drive = v; Some(ParamUpdate::Changed("high_drive", v)) }
        MultibandSaturatorMessage::LowLevelChanged(v) => { cfg.low_level = v; Some(ParamUpdate::Changed("low_level", v)) }
        MultibandSaturatorMessage::MidLevelChanged(v) => { cfg.mid_level = v; Some(ParamUpdate::Changed("mid_level", v)) }
        MultibandSaturatorMessage::HighLevelChanged(v) => { cfg.high_level = v; Some(ParamUpdate::Changed("high_level", v)) }
        MultibandSaturatorMessage::LowFreqChanged(v) => { cfg.low_freq = v; Some(ParamUpdate::Changed("low_freq", v)) }
        MultibandSaturatorMessage::HighFreqChanged(v) => { cfg.high_freq = v; Some(ParamUpdate::Changed("high_freq", v)) }
    }
}

// --- View ---

pub fn view(
    idx: usize,
    cfg: &MultibandSaturatorConfig,
    state: StageViewState,
) -> Element<'_, Message> {
    stage_card(
        tr!(stage_multiband_saturator),
        idx,
        state,
        || {
            let crossover_section = column![
                text(tr!(crossover)).size(TEXT_SIZE_INFO),
                labeled_slider(
                    tr!(low_freq),
                    50.0..=500.0,
                    cfg.low_freq,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::LowFreqChanged(v)
                        )
                    ),
                    |v| format!("{v:.0} {}", tr!(hz)),
                    1.0
                ),
                labeled_slider(
                    tr!(high_freq),
                    1000.0..=6000.0,
                    cfg.high_freq,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::HighFreqChanged(v)
                        )
                    ),
                    |v| format!("{v:.0} {}", tr!(hz)),
                    10.0
                ),
            ]
            .spacing(SPACING_TIGHT);

            let low_band_section = column![
                text(tr!(low_band)).size(TEXT_SIZE_INFO),
                labeled_slider(
                    tr!(drive),
                    0.0..=1.0,
                    cfg.low_drive,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::LowDriveChanged(v)
                        )
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(level),
                    0.0..=2.0,
                    cfg.low_level,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::LowLevelChanged(v)
                        )
                    ),
                    |v| format!("{v:.2}"),
                    0.01
                ),
            ]
            .spacing(SPACING_TIGHT);

            let mid_band_section = column![
                text(tr!(mid_band)).size(TEXT_SIZE_INFO),
                labeled_slider(
                    tr!(drive),
                    0.0..=1.0,
                    cfg.mid_drive,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::MidDriveChanged(v)
                        )
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(level),
                    0.0..=2.0,
                    cfg.mid_level,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::MidLevelChanged(v)
                        )
                    ),
                    |v| format!("{v:.2}"),
                    0.01
                ),
            ]
            .spacing(SPACING_TIGHT);

            let high_band_section = column![
                text(tr!(high_band)).size(TEXT_SIZE_INFO),
                labeled_slider(
                    tr!(drive),
                    0.0..=1.0,
                    cfg.high_drive,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::HighDriveChanged(v)
                        )
                    ),
                    |v| format!("{:.0}%", v * 100.0),
                    0.01
                ),
                labeled_slider(
                    tr!(level),
                    0.0..=2.0,
                    cfg.high_level,
                    move |v| Message::Stage(
                        idx,
                        StageMessage::MultibandSaturator(
                            MultibandSaturatorMessage::HighLevelChanged(v)
                        )
                    ),
                    |v| format!("{v:.2}"),
                    0.01
                ),
            ]
            .spacing(SPACING_TIGHT);

            let bands_row = row![low_band_section, mid_band_section, high_band_section]
                .spacing(SPACING_SECTION)
                .width(Length::Fill);

            column![crossover_section, bands_row]
                .spacing(SPACING_NORMAL)
                .into()
        },
    )
}
