use iced::widget::{container, row, space, text};
use iced::{Color, Element, Length};

use crate::audio::peak_meter::PeakMeterInfo;
use crate::gui::messages::Message;
use crate::tr;

const METER_WIDTH: f32 = 200.0;
const METER_HEIGHT: f32 = 20.0;

pub struct PeakMeterDisplay {
    info: PeakMeterInfo,
    xrun_count: u64,
    cpu_load: f32,
}

impl Default for PeakMeterDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl PeakMeterDisplay {
    pub fn new() -> Self {
        Self {
            info: PeakMeterInfo::default(),
            xrun_count: 0,
            cpu_load: 0.0,
        }
    }

    pub const fn update(&mut self, info: PeakMeterInfo, xrun_count: u64, cpu_load: f32) {
        self.info = info;
        self.xrun_count = xrun_count;
        self.cpu_load = cpu_load;
    }

    pub fn view(&self) -> Element<'_, Message> {
        let level_pct = ((self.info.peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
        let level_width = METER_WIDTH * level_pct;

        let color = if self.info.is_clipping {
            Color::from_rgb(1.0, 0.0, 0.0) // Red for clipping
        } else if self.info.peak_db > -6.0 {
            Color::from_rgb(1.0, 0.7, 0.0) // Orange/yellow for hot
        } else if self.info.peak_db > -20.0 {
            Color::from_rgb(0.0, 1.0, 0.0) // Green for normal
        } else {
            Color::from_rgb(0.0, 0.5, 0.0) // Dark green for quiet
        };

        let db_text = if self.info.peak_db > -100.0 {
            format!("{:+.1} {}", self.info.peak_db, tr!(db))
        } else {
            format!("-∞ {}", tr!(db))
        };

        let status_text = if self.info.is_clipping {
            text("CLIP!")
                .size(14)
                .style(move |_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
                })
        } else {
            text("")
                .size(14)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                })
        };

        let meter = container(
            container(space().width(level_width).height(METER_HEIGHT))
                .style(move |_| container::Style::default().background(color)),
        )
        .width(Length::Fixed(METER_WIDTH))
        .height(Length::Fixed(METER_HEIGHT))
        .style(|_| {
            container::Style::default()
                .background(Color::from_rgb(0.2, 0.2, 0.2))
                .border(iced::Border::default().width(1).rounded(3))
        });

        let xrun_color = if self.xrun_count > 0 {
            Color::from_rgb(1.0, 0.3, 0.3)
        } else {
            Color::from_rgb(0.5, 0.5, 0.5)
        };

        let cpu_color = if self.cpu_load > 80.0 {
            Color::from_rgb(1.0, 0.0, 0.0)
        } else if self.cpu_load > 50.0 {
            Color::from_rgb(1.0, 0.7, 0.0)
        } else {
            Color::from_rgb(0.5, 0.5, 0.5)
        };

        let xrun_count = self.xrun_count;
        let cpu_load = self.cpu_load;

        row![
            text(tr!(output)).width(Length::Fixed(75.0)),
            meter,
            text(db_text)
                .size(14)
                .width(Length::Fixed(80.0))
                .style(move |_: &iced::Theme| iced::widget::text::Style { color: Some(color) }),
            status_text.width(Length::Fixed(50.0)),
            text(format!("{}: {xrun_count}", tr!(xruns)))
                .size(14)
                .width(Length::Fixed(80.0))
                .style(move |_: &iced::Theme| iced::widget::text::Style {
                    color: Some(xrun_color),
                }),
            text(format!("{}: {cpu_load:.0}%", tr!(cpu)))
                .size(14)
                .width(Length::Fixed(80.0))
                .style(move |_: &iced::Theme| iced::widget::text::Style {
                    color: Some(cpu_color),
                }),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .into()
    }
}
