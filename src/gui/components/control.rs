use iced::widget::{button, container, pick_list, row, text};
use iced::{Alignment, Element, Length};

use crate::gui::components::ICONS_FONT;
use crate::gui::config::StageType;
use crate::gui::messages::Message;

pub struct Control {
    selected_stage_type: StageType,
    is_recording: bool,
}

impl Control {
    pub fn new(selected_stage_type: StageType, is_recording: bool) -> Self {
        Self {
            selected_stage_type,
            is_recording,
        }
    }

    pub fn view(&self) -> Element<'static, Message> {
        let stage_types = vec![
            StageType::Filter,
            StageType::Preamp,
            StageType::Compressor,
            StageType::ToneStack,
            StageType::PowerAmp,
            StageType::Level,
        ];

        let stage_controls = row![
            pick_list(
                stage_types,
                Some(self.selected_stage_type),
                Message::StageTypeSelected
            ),
            button("Add Stage").on_press(Message::AddStage),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Recording controls
        let record_button = if self.is_recording {
            button(text("Stop Recording").font(ICONS_FONT))
                .on_press(Message::StopRecording)
                .style(iced::widget::button::danger)
        } else {
            button(text("Start Recording").font(ICONS_FONT))
                .on_press(Message::StartRecording)
                .style(iced::widget::button::success)
        };

        let recording_status = if self.is_recording {
            text("Recording...").style(|_| iced::widget::text::Style {
                color: Some(iced::Color::from_rgb(1.0, 0.3, 0.3)),
            })
        } else {
            text("").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
        };

        let recording_controls = container(
            row![record_button, recording_status]
                .spacing(10)
                .align_y(Alignment::Center),
        )
        .padding(5);

        row![
            stage_controls,
            iced::widget::horizontal_space(),
            recording_controls
        ]
        .spacing(20)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    }
}
