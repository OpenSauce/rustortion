use iced::widget::{column, scrollable};
use iced::{Element, Length};

use crate::gui::messages::Message;
use crate::gui::stages::StageConfig;

pub struct StageList {
    stages: Vec<StageConfig>,
}

impl StageList {
    pub fn new(stages: Vec<StageConfig>) -> Self {
        Self { stages }
    }

    pub fn set_stages(&mut self, stages: &[StageConfig]) {
        self.stages = stages.to_vec();
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut col = column![].width(Length::Fill).padding(10);

        for (idx, stage) in self.stages.iter().enumerate() {
            col = col.push(stage.view(idx, self.stages.len()));
        }

        scrollable(col).height(Length::FillPortion(9)).into()
    }
}
