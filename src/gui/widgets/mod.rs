pub mod compressor;
pub mod filter;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

use crate::gui::amp::Message;
use iced::{Element, Length};

pub fn labeled_slider<'a, F: 'a + Fn(f32) -> Message>(
    label: &'a str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    on_change: F,
    format: impl Fn(f32) -> String + 'a,
) -> Element<'a, Message> {
    use iced::Alignment;
    use iced::widget::{row, slider, text};

    row![
        text(label).width(Length::FillPortion(3)),
        slider(range, value, on_change).width(Length::FillPortion(5)),
        text(format(value)).width(Length::FillPortion(2)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}
