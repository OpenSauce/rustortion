use iced::widget::{column, pick_list, row, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

use crate::amp::stages::filter::{FilterStage, FilterType};
use crate::gui::components::widgets::common::{labeled_slider, stage_card};
use crate::gui::messages::Message;
use crate::tr;

use super::StageMessage;

// --- Config ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FilterConfig {
    pub filter_type: FilterType,
    pub cutoff_hz: f32,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Highpass,
            cutoff_hz: 100.0,
        }
    }
}

impl FilterConfig {
    pub fn to_stage(&self, sample_rate: f32) -> FilterStage {
        FilterStage::new(self.filter_type, self.cutoff_hz, sample_rate)
    }

    pub const fn apply(&mut self, msg: FilterMessage) {
        match msg {
            FilterMessage::TypeChanged(t) => self.filter_type = t,
            FilterMessage::CutoffChanged(v) => self.cutoff_hz = v,
        }
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub enum FilterMessage {
    TypeChanged(FilterType),
    CutoffChanged(f32),
}

// --- View ---

const FILTER_TYPES: [FilterType; 2] = [FilterType::Highpass, FilterType::Lowpass];

pub fn view(
    idx: usize,
    cfg: &FilterConfig,
    total_stages: usize,
    is_collapsed: bool,
) -> Element<'_, Message> {
    stage_card(tr!(stage_filter), idx, total_stages, is_collapsed, || {
        let type_picker = row![
            text(tr!(type_label)).width(Length::FillPortion(3)),
            pick_list(FILTER_TYPES, Some(cfg.filter_type), move |t| {
                Message::Stage(idx, StageMessage::Filter(FilterMessage::TypeChanged(t)))
            })
            .width(Length::FillPortion(7)),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let range = match cfg.filter_type {
            FilterType::Highpass => 0.0..=1000.0,
            FilterType::Lowpass => 5000.0..=15000.0,
        };

        column![
            type_picker,
            labeled_slider(
                tr!(cutoff),
                range,
                cfg.cutoff_hz,
                move |v| Message::Stage(
                    idx,
                    StageMessage::Filter(FilterMessage::CutoffChanged(v))
                ),
                |v| format!("{v:.0} {}", tr!(hz)),
                1.0
            ),
        ]
        .spacing(5)
        .into()
    })
}
