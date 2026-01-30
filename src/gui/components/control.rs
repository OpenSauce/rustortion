use iced::widget::{button, container, pick_list, row, space, text};
use iced::{Alignment, Element, Length};

use crate::gui::config::StageType;
use crate::gui::messages::Message;
use crate::tr;

pub struct Control {
    selected_stage_type: StageType,
}

const STAGE_TYPES: &[StageType] = &[
    StageType::Filter,
    StageType::Preamp,
    StageType::Compressor,
    StageType::ToneStack,
    StageType::PowerAmp,
    StageType::Level,
    StageType::NoiseGate,
    StageType::MultibandSaturator,
];

impl Control {
    pub fn new(selected_stage_type: StageType) -> Self {
        Self {
            selected_stage_type,
        }
    }

    pub fn set_selected_stage_type(&mut self, ty: StageType) {
        self.selected_stage_type = ty;
    }

    pub fn view(&self, is_recording: bool) -> Element<'_, Message> {
        let stage_controls = row![
            pick_list(
                STAGE_TYPES,
                Some(self.selected_stage_type),
                Message::StageTypeSelected
            ),
            button(tr!(add_stage)).on_press(Message::AddStage),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Recording controls
        let record_button = if is_recording {
            button(text(tr!(stop_recording)))
                .on_press(Message::StopRecording)
                .style(iced::widget::button::danger)
        } else {
            button(text(tr!(start_recording)))
                .on_press(Message::StartRecording)
                .style(iced::widget::button::success)
        };

        let recording_status = if is_recording {
            text(tr!(recording)).style(|_| iced::widget::text::Style {
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

        row![stage_controls, space::horizontal(), recording_controls]
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
}
