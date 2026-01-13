use iced::widget::{button, container, pick_list, row, space, text};
use iced::{Alignment, Element, Length};

use crate::gui::config::StageType;
use crate::gui::messages::Message;

pub struct Control {
    selected_stage_type: StageType,
    is_recording: bool,
    is_looping: bool,
}

const STAGE_TYPES: &[StageType] = &[
    StageType::Filter,
    StageType::Preamp,
    StageType::Compressor,
    StageType::ToneStack,
    StageType::PowerAmp,
    StageType::Level,
    StageType::NoiseGate,
];

impl Control {
    pub fn new(selected_stage_type: StageType) -> Self {
        Self {
            selected_stage_type,
            is_recording: false,
            is_looping: false,
        }
    }

    pub fn set_selected_stage_type(&mut self, ty: StageType) {
        self.selected_stage_type = ty;
    }

    pub fn view(&self) -> Element<'_, Message> {
        let stage_controls = row![
            pick_list(
                STAGE_TYPES,
                Some(self.selected_stage_type),
                Message::StageTypeSelected
            ),
            button("Add Stage").on_press(Message::AddStage),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Looping controls
        let looping_button = if self.is_looping {
            button(text("Stop Looping"))
                .on_press(Message::StopLooping)
                .style(iced::widget::button::danger)
        } else {
            button(text("Start Looping"))
                .on_press(Message::StartLooping)
                .style(iced::widget::button::success)
        };

        let looping_status = if self.is_looping {
            text("Looping...").style(|_| iced::widget::text::Style {
                color: Some(iced::Color::from_rgb(1.0, 0.3, 0.3)),
            })
        } else {
            text("").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
        };

        let looping_controls = container(
            row![looping_button, looping_status]
                .spacing(10)
                .align_y(Alignment::Center),
        )
        .padding(5);

        // Recording controls
        let record_button = if self.is_recording {
            button(text("Stop Recording"))
                .on_press(Message::StopRecording)
                .style(iced::widget::button::danger)
        } else {
            button(text("Start Recording"))
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
            space::horizontal(),
            looping_controls,
            recording_controls
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    pub fn set_selected(&mut self, t: StageType) {
        self.selected_stage_type = t;
    }
    pub fn selected(&self) -> StageType {
        self.selected_stage_type
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.is_recording = recording;
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.is_looping = looping;
    }
}
