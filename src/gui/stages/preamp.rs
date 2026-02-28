use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::clipper::ClipperType;
use crate::amp::stages::preamp::PreampStage;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PreampConfig {
    pub gain: f32,
    pub bias: f32,
    pub clipper_type: ClipperType,
}

impl Default for PreampConfig {
    fn default() -> Self {
        Self {
            gain: 5.0,
            bias: 0.0,
            clipper_type: ClipperType::Soft,
        }
    }
}

impl PreampConfig {
    pub fn to_stage(&self, sample_rate: f32) -> PreampStage {
        PreampStage::new(self.gain, self.bias, self.clipper_type, sample_rate)
    }

    pub fn apply(&mut self, msg: PreampMessage) {
        match msg {
            PreampMessage::GainChanged(v) => self.gain = v,
            PreampMessage::BiasChanged(v) => self.bias = v,
            PreampMessage::ClipperChanged(c) => self.clipper_type = c,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum PreampMessage {
    GainChanged(f32),
    BiasChanged(f32),
    ClipperChanged(ClipperType),
}

// --- View ---

const CLIPPER_TYPES: [ClipperType; 5] = [
    ClipperType::Soft,
    ClipperType::Medium,
    ClipperType::Hard,
    ClipperType::Asymmetric,
    ClipperType::ClassA,
];

pub fn view(
    idx: usize,
    cfg: &PreampConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_preamp), idx, total_stages, is_collapsed);

    let mut content = column![header].spacing(5);

    if !is_collapsed {
        let clipper_picker = row![
            text(tr!(clipper)).width(Length::FillPortion(3)),
            pick_list(CLIPPER_TYPES, Some(cfg.clipper_type), move |t| {
                Message::Stage(idx, StageMessage::Preamp(PreampMessage::ClipperChanged(t)))
            })
            .width(Length::FillPortion(7)),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let body = column![
            clipper_picker,
            labeled_slider(
                tr!(gain),
                0.0..=10.0,
                cfg.gain,
                move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::GainChanged(v))),
                |v| format!("{v:.1}"),
                0.1
            ),
            labeled_slider(
                tr!(bias),
                -1.0..=1.0,
                cfg.bias,
                move |v| Message::Stage(idx, StageMessage::Preamp(PreampMessage::BiasChanged(v))),
                |v| format!("{v:.2}"),
                0.1
            ),
        ]
        .spacing(5);

        content = content.push(body);
    }

    let padding = if is_collapsed { 5 } else { 10 };

    container(content.padding(padding))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
