use iced::widget::{column, scrollable};
use iced::{Element, Length};

use crate::gui::components::stages;
use crate::gui::config::StageConfig;
use crate::gui::messages::Message;

pub struct StageList<'a> {
    stages: &'a [StageConfig],
}

impl<'a> StageList<'a> {
    pub fn new(stages: &'a [StageConfig]) -> Self {
        Self { stages }
    }

    pub fn view(&self) -> Element<'a, Message> {
        let mut col = column![].spacing(10).width(Length::Fill);

        for (idx, stage) in self.stages.iter().enumerate() {
            let widget = match stage {
                StageConfig::Filter(cfg) => stages::filter::view(idx, cfg, self.stages.len()),
                StageConfig::Preamp(cfg) => stages::preamp::view(idx, cfg, self.stages.len()),
                StageConfig::Compressor(cfg) => {
                    stages::compressor::view(idx, cfg, self.stages.len())
                }
                StageConfig::ToneStack(cfg) => stages::tonestack::view(idx, cfg, self.stages.len()),
                StageConfig::PowerAmp(cfg) => stages::poweramp::view(idx, cfg, self.stages.len()),
                StageConfig::Level(cfg) => stages::level::view(idx, cfg, self.stages.len()),
            };
            col = col.push(widget);
        }

        scrollable(col).height(Length::FillPortion(9)).into()
    }
}
