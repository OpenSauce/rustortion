use iced::widget::{column, container, row, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::multiband_saturator::MultibandSaturatorStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MultibandSaturatorConfig {
    pub low_drive: f32,
    pub mid_drive: f32,
    pub high_drive: f32,
    pub low_level: f32,
    pub mid_level: f32,
    pub high_level: f32,
    pub low_freq: f32,
    pub high_freq: f32,
}

impl Default for MultibandSaturatorConfig {
    fn default() -> Self {
        Self {
            low_drive: 0.3,
            mid_drive: 0.5,
            high_drive: 0.4,
            low_level: 1.0,
            mid_level: 1.0,
            high_level: 1.0,
            low_freq: 200.0,
            high_freq: 2500.0,
        }
    }
}

impl MultibandSaturatorConfig {
    pub fn to_stage(&self, sample_rate: f32) -> MultibandSaturatorStage {
        MultibandSaturatorStage::new(
            self.low_drive,
            self.mid_drive,
            self.high_drive,
            self.low_level,
            self.mid_level,
            self.high_level,
            self.low_freq,
            self.high_freq,
            sample_rate,
        )
    }

    pub fn apply(&mut self, msg: MultibandSaturatorMessage) {
        match msg {
            MultibandSaturatorMessage::LowDriveChanged(v) => self.low_drive = v,
            MultibandSaturatorMessage::MidDriveChanged(v) => self.mid_drive = v,
            MultibandSaturatorMessage::HighDriveChanged(v) => self.high_drive = v,
            MultibandSaturatorMessage::LowLevelChanged(v) => self.low_level = v,
            MultibandSaturatorMessage::MidLevelChanged(v) => self.mid_level = v,
            MultibandSaturatorMessage::HighLevelChanged(v) => self.high_level = v,
            MultibandSaturatorMessage::LowFreqChanged(v) => self.low_freq = v,
            MultibandSaturatorMessage::HighFreqChanged(v) => self.high_freq = v,
        }
    }
}

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

// --- View ---

pub fn view(
    idx: usize,
    cfg: &MultibandSaturatorConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    let header = stage_header(
        tr!(stage_multiband_saturator),
        idx,
        total_stages,
        is_collapsed,
    );

    let mut content = column![header].spacing(5);

    if !is_collapsed {
        let crossover_section = column![
            text(tr!(crossover)).size(14),
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
        .spacing(5);

        let low_band_section = column![
            text(tr!(low_band)).size(14),
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
        .spacing(5);

        let mid_band_section = column![
            text(tr!(mid_band)).size(14),
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
        .spacing(5);

        let high_band_section = column![
            text(tr!(high_band)).size(14),
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
        .spacing(5);

        let bands_row = row![low_band_section, mid_band_section, high_band_section]
            .spacing(20)
            .width(Length::Fill);

        let body = column![crossover_section, bands_row].spacing(10);

        content = content.push(body);
    }

    let padding = if is_collapsed { 5 } else { 10 };

    container(content.padding(padding))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
