use iced::widget::{column, scrollable};
use iced::{Element, Length};

use crate::gui::messages::Message;
use crate::gui::stages::StageConfig;

pub struct StageList {
    stages: Vec<StageConfig>,
}

impl StageList {
    pub const fn new(stages: Vec<StageConfig>) -> Self {
        Self { stages }
    }

    pub fn set_stages(&mut self, stages: &[StageConfig]) {
        self.stages = stages.to_vec();
    }

    pub fn view(&self, collapsed: &[bool]) -> Element<'_, Message> {
        let mut col = column![].width(Length::Fill).padding(10);

        for (idx, stage) in self.stages.iter().enumerate() {
            let is_collapsed = collapsed.get(idx).copied().unwrap_or(false);
            col = col.push(stage.view(idx, self.stages.len(), is_collapsed));
        }

        scrollable(col).height(Length::FillPortion(9)).into()
    }
}
