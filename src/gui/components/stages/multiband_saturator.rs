use iced::widget::{column, container, row, text};
use iced::{Element, Length};

use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::MultibandSaturatorConfig;
use crate::gui::messages::{Message, MultibandSaturatorMessage, StageMessage};
use crate::tr;

pub fn view(
    idx: usize,
    cfg: &MultibandSaturatorConfig,
    total_stages: usize,
) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_multiband_saturator), idx, total_stages);

    // Crossover frequency controls
    let crossover_section = column![
        text("Crossover").size(14),
        labeled_slider(
            tr!(low_freq),
            50.0..=500.0,
            cfg.low_freq,
            move |v| Message::Stage(
                idx,
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::LowFreqChanged(v))
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
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::HighFreqChanged(v))
            ),
            |v| format!("{v:.0} {}", tr!(hz)),
            10.0
        ),
    ]
    .spacing(5);

    // Low band controls
    let low_band_section = column![
        text(tr!(low_band)).size(14),
        labeled_slider(
            tr!(drive),
            0.0..=1.0,
            cfg.low_drive,
            move |v| Message::Stage(
                idx,
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::LowDriveChanged(v))
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
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::LowLevelChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.01
        ),
    ]
    .spacing(5);

    // Mid band controls
    let mid_band_section = column![
        text(tr!(mid_band)).size(14),
        labeled_slider(
            tr!(drive),
            0.0..=1.0,
            cfg.mid_drive,
            move |v| Message::Stage(
                idx,
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::MidDriveChanged(v))
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
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::MidLevelChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.01
        ),
    ]
    .spacing(5);

    // High band controls
    let high_band_section = column![
        text(tr!(high_band)).size(14),
        labeled_slider(
            tr!(drive),
            0.0..=1.0,
            cfg.high_drive,
            move |v| Message::Stage(
                idx,
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::HighDriveChanged(v))
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
                StageMessage::MultibandSaturator(MultibandSaturatorMessage::HighLevelChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.01
        ),
    ]
    .spacing(5);

    // Layout: crossover on top, then three band sections side by side
    let bands_row = row![low_band_section, mid_band_section, high_band_section]
        .spacing(20)
        .width(Length::Fill);

    let body = column![crossover_section, bands_row].spacing(10);

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
