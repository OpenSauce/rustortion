// src/bin/gui.rs
// -----------------------------------------------------------------------------
// Modern Iced v0.13 GUI: lets you add **Filter** stages and tweak two
// knobs (cutoff + resonance). Uses the `application` function pattern
// with Task support for better async handling.
// Build & run with:
//     cargo run --bin gui
// -----------------------------------------------------------------------------

use iced::Length::Fill;
use iced::widget::{button, column, container, row, scrollable, slider, text};
use iced::{Alignment, Element, Length, Task};

use rustortion::sim::chain::AmplifierChain;
use rustortion::sim::stages::Stage;
use rustortion::sim::stages::filter::{FilterStage, FilterType};

pub fn main() -> iced::Result {
    iced::application("Rustortion", AmplifierGui::update, AmplifierGui::view).run()
}

// -----------------------------------------------------------------------------
// Core editor state -----------------------------------------------------------
// -----------------------------------------------------------------------------

#[derive(Debug, Default)]
struct AmplifierGui {
    stages: Vec<FilterConfig>,
}

#[derive(Debug, Clone, Copy)]
struct FilterConfig {
    filter_type: FilterType,
    cutoff_hz: f32,
    resonance: f32,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Highpass,
            cutoff_hz: 100.0,
            resonance: 0.0,
        }
    }
}

// -----------------------------------------------------------------------------
// Messages --------------------------------------------------------------------
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Message {
    AddStage,
    RemoveStage(usize),
    FilterTypeChanged(usize, FilterType),
    CutoffChanged(usize, f32),
    ResonanceChanged(usize, f32),
    Start,
}

// -----------------------------------------------------------------------------
// Application impl (modern Iced app pattern) ----------------------------------
// -----------------------------------------------------------------------------

impl AmplifierGui {
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Start => {
                let sample_rate = 44100.0; // Example sample rate
                let chain = self.to_amp_chain(sample_rate);
                println!("Starting!");
                Task::none()
            }
            Message::AddStage => {
                self.stages.push(FilterConfig::default());
                Task::none()
            }
            Message::RemoveStage(i) if i < self.stages.len() => {
                self.stages.remove(i);
                Task::none()
            }
            Message::FilterTypeChanged(i, filter_type) => {
                if let Some(cfg) = self.stages.get_mut(i) {
                    cfg.filter_type = filter_type;
                }
                Task::none()
            }
            Message::CutoffChanged(i, v) => {
                if let Some(cfg) = self.stages.get_mut(i) {
                    cfg.cutoff_hz = v.clamp(20.0, 20_000.0);
                }
                Task::none()
            }
            Message::ResonanceChanged(i, v) => {
                if let Some(cfg) = self.stages.get_mut(i) {
                    cfg.resonance = v.clamp(0.0, 1.0);
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        let mut list = column![].spacing(10).width(Length::Fill);

        // Add all stages to the list
        for (idx, cfg) in self.stages.iter().enumerate() {
            list = list.push(stage_widget(idx, cfg));
        }

        let scrollable_content = scrollable(list);

        let footer = row![
            button("Add Filter Stage").on_press(Message::AddStage),
            button("Start").on_press(Message::Start),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(column![scrollable_content, footer].spacing(20).padding(20))
            .width(Length::Fill)
            .center(Fill)
            .into()
    }
}

// -----------------------------------------------------------------------------
// Per‑stage UI ----------------------------------------------------------------
// -----------------------------------------------------------------------------

fn stage_widget(idx: usize, cfg: &FilterConfig) -> Element<Message> {
    let header = row![
        text(format!("Filter {idx}")),
        button("✕").on_press(Message::RemoveStage(idx)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Filter type picker would go here
    // For now, we're just showing the current type
    let filter_type_text = text(format!("Type: {:?}", cfg.filter_type));

    let body = column![
        filter_type_text,
        slider_row("Cutoff (Hz)", 20.0..=20_000.0, cfg.cutoff_hz, move |v| {
            Message::CutoffChanged(idx, v)
        }),
        slider_row("Resonance", 0.0..=1.0, cfg.resonance, move |v| {
            Message::ResonanceChanged(idx, v)
        }),
    ]
    .spacing(5);

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .into()
}

fn slider_row<'a, F: 'a + Fn(f32) -> Message>(
    label: &str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    on_change: F,
) -> Element<'a, Message> {
    row![
        text(label.to_owned()).width(Length::FillPortion(2)),
        slider(range, value, on_change).width(Length::FillPortion(5)),
        text(format!("{value:.1}")),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

impl FilterConfig {
    fn to_stage(self, sample_rate: f32) -> Box<dyn Stage + Send> {
        Box::new(FilterStage::new(
            "UI Filter",
            self.filter_type,
            self.cutoff_hz,
            self.resonance,
            sample_rate,
        ))
    }
}

impl AmplifierGui {
    pub fn to_amp_chain(&self, sample_rate: f32) -> AmplifierChain {
        let mut chain = AmplifierChain::new("Custom Filter Chain");

        for (idx, config) in self.stages.iter().enumerate() {
            chain.add_stage(Box::new(FilterStage::new(
                &format!("Filter {}", idx),
                config.filter_type,
                config.cutoff_hz,
                config.resonance,
                sample_rate,
            )));
        }

        if !self.stages.is_empty() {
            let stage_indices: Vec<usize> = (0..self.stages.len()).collect();
            chain.define_channel(0, Vec::new(), stage_indices, Vec::new());
            chain.set_channel(0);
        }

        chain
    }
}
