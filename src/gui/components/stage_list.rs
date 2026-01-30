use iced::widget::{column, scrollable};
use iced::{Element, Length};

use crate::gui::components::stages;
use crate::gui::config::StageConfig;
use crate::gui::messages::Message;

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
            let widget = match stage {
                StageConfig::Filter(cfg) => stages::filter::view(idx, cfg, self.stages.len()),
                StageConfig::Preamp(cfg) => stages::preamp::view(idx, cfg, self.stages.len()),
                StageConfig::Compressor(cfg) => {
                    stages::compressor::view(idx, cfg, self.stages.len())
                }
                StageConfig::ToneStack(cfg) => stages::tonestack::view(idx, cfg, self.stages.len()),
                StageConfig::PowerAmp(cfg) => stages::poweramp::view(idx, cfg, self.stages.len()),
                StageConfig::Level(cfg) => stages::level::view(idx, cfg, self.stages.len()),
                StageConfig::NoiseGate(cfg) => {
                    stages::noise_gate::view(idx, cfg, self.stages.len())
                }
                StageConfig::MultibandSaturator(cfg) => {
                    stages::multiband_saturator::view(idx, cfg, self.stages.len())
                }
            };
            col = col.push(widget);
        }

        scrollable(col).height(Length::FillPortion(9)).into()
    }
}
