use iced::widget::{button, pick_list, row};
use iced::{Alignment, Element};

use crate::gui::config::StageType;
use crate::gui::messages::Message;

pub struct Control {
    selected_stage_type: StageType,
}

impl Control {
    pub fn new(selected_stage_type: StageType) -> Self {
        Self {
            selected_stage_type,
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

        row![
            pick_list(
                stage_types,
                Some(self.selected_stage_type),
                Message::StageTypeSelected
            ),
            button("Add Stage").on_press(Message::AddStage),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }
}
