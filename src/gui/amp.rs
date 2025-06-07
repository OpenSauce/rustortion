use iced::Length::Fill;
use iced::widget::{button, column, container, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Task, Theme};

use crate::io::manager::ProcessorManager;
use crate::sim::chain::AmplifierChain;
use crate::sim::stages::{
    clipper::ClipperType,
    compressor::CompressorStage,
    filter::{FilterStage, FilterType},
    poweramp::{PowerAmpStage, PowerAmpType},
    preamp::PreampStage,
    tonestack::{ToneStackModel, ToneStackStage},
};

use crate::gui::widgets::{compressor, filter, poweramp, preamp, tonestack};

pub fn start(processor_manager: ProcessorManager) -> iced::Result {
    iced::application("Rustortion", AmplifierGui::update, AmplifierGui::view)
        .window_size((800.0, 600.0))
        .theme(AmplifierGui::theme)
        .run_with(move || {
            (
                AmplifierGui {
                    processor_manager,
                    stages: Vec::new(),
                    selected_stage_type: StageType::default(),
                },
                Task::none(),
            )
        })
}

// Stage type enum
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageType {
    #[default]
    Filter,
    Preamp,
    Compressor,
    ToneStack,
    PowerAmp,
}

impl std::fmt::Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageType::Filter => write!(f, "Filter"),
            StageType::Preamp => write!(f, "Preamp"),
            StageType::Compressor => write!(f, "Compressor"),
            StageType::ToneStack => write!(f, "Tone Stack"),
            StageType::PowerAmp => write!(f, "Power Amp"),
        }
    }
}

// Stage configurations
#[derive(Debug, Clone)]
pub enum StageConfig {
    Filter(FilterConfig),
    Preamp(PreampConfig),
    Compressor(CompressorConfig),
    ToneStack(ToneStackConfig),
    PowerAmp(PowerAmpConfig),
}

#[derive(Debug, Clone, Copy)]
pub struct FilterConfig {
    pub filter_type: FilterType,
    pub cutoff_hz: f32,
    pub resonance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PreampConfig {
    pub gain: f32,
    pub bias: f32,
    pub clipper_type: ClipperType,
}

#[derive(Debug, Clone, Copy)]
pub struct CompressorConfig {
    pub attack_ms: f32,
    pub release_ms: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub makeup_db: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ToneStackConfig {
    pub model: ToneStackModel,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerAmpConfig {
    pub drive: f32,
    pub amp_type: PowerAmpType,
    pub sag: f32,
}

// Default implementations
impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Highpass,
            cutoff_hz: 100.0,
            resonance: 0.0,
        }
    }
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

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            attack_ms: 1.0,
            release_ms: 100.0,
            threshold_db: -20.0,
            ratio: 4.0,
            makeup_db: 0.0,
        }
    }
}

impl Default for ToneStackConfig {
    fn default() -> Self {
        Self {
            model: ToneStackModel::Modern,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
        }
    }
}

impl Default for PowerAmpConfig {
    fn default() -> Self {
        Self {
            drive: 0.5,
            amp_type: PowerAmpType::ClassAB,
            sag: 0.3,
        }
    }
}

// Main GUI state
#[derive(Debug)]
struct AmplifierGui {
    processor_manager: ProcessorManager,
    stages: Vec<StageConfig>,
    selected_stage_type: StageType,
}

// Messages
#[derive(Debug, Clone)]
pub enum Message {
    AddStage,
    RemoveStage(usize),
    MoveStageUp(usize),
    MoveStageDown(usize),
    StageTypeSelected(StageType),

    // Filter messages
    FilterTypeChanged(usize, FilterType),
    FilterCutoffChanged(usize, f32),
    FilterResonanceChanged(usize, f32),

    // Preamp messages
    PreampGainChanged(usize, f32),
    PreampBiasChanged(usize, f32),
    PreampClipperChanged(usize, ClipperType),

    // Compressor messages
    CompressorThresholdChanged(usize, f32),
    CompressorRatioChanged(usize, f32),
    CompressorAttackChanged(usize, f32),
    CompressorReleaseChanged(usize, f32),
    CompressorMakeupChanged(usize, f32),

    // ToneStack messages
    ToneStackModelChanged(usize, ToneStackModel),
    ToneStackBassChanged(usize, f32),
    ToneStackMidChanged(usize, f32),
    ToneStackTrebleChanged(usize, f32),
    ToneStackPresenceChanged(usize, f32),

    // PowerAmp messages
    PowerAmpTypeChanged(usize, PowerAmpType),
    PowerAmpDriveChanged(usize, f32),
    PowerAmpSagChanged(usize, f32),
}

// Application implementation
impl AmplifierGui {
    fn update(&mut self, msg: Message) -> Task<Message> {
        let mut should_update_chain = false;

        match msg {
            Message::AddStage => {
                let new_stage = match self.selected_stage_type {
                    StageType::Filter => StageConfig::Filter(FilterConfig::default()),
                    StageType::Preamp => StageConfig::Preamp(PreampConfig::default()),
                    StageType::Compressor => StageConfig::Compressor(CompressorConfig::default()),
                    StageType::ToneStack => StageConfig::ToneStack(ToneStackConfig::default()),
                    StageType::PowerAmp => StageConfig::PowerAmp(PowerAmpConfig::default()),
                };
                self.stages.push(new_stage);
                should_update_chain = true;
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                }
                should_update_chain = true;
            }
            Message::MoveStageUp(idx) => {
                if idx > 0 && idx < self.stages.len() {
                    self.stages.swap(idx - 1, idx);
                    should_update_chain = true;
                }
            }
            Message::MoveStageDown(idx) => {
                if idx < self.stages.len().saturating_sub(1) {
                    self.stages.swap(idx, idx + 1);
                    should_update_chain = true;
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.selected_stage_type = stage_type;
            }

            // Filter updates
            Message::FilterTypeChanged(idx, filter_type) => {
                if let Some(StageConfig::Filter(cfg)) = self.stages.get_mut(idx) {
                    cfg.filter_type = filter_type;
                }
                should_update_chain = true;
            }
            Message::FilterCutoffChanged(idx, v) => {
                if let Some(StageConfig::Filter(cfg)) = self.stages.get_mut(idx) {
                    cfg.cutoff_hz = v;
                }
                should_update_chain = true;
            }
            Message::FilterResonanceChanged(idx, v) => {
                if let Some(StageConfig::Filter(cfg)) = self.stages.get_mut(idx) {
                    cfg.resonance = v;
                }
                should_update_chain = true;
            }

            // Preamp updates
            Message::PreampGainChanged(idx, v) => {
                if let Some(StageConfig::Preamp(cfg)) = self.stages.get_mut(idx) {
                    cfg.gain = v;
                }
                should_update_chain = true;
            }
            Message::PreampBiasChanged(idx, v) => {
                if let Some(StageConfig::Preamp(cfg)) = self.stages.get_mut(idx) {
                    cfg.bias = v;
                }
                should_update_chain = true;
            }
            Message::PreampClipperChanged(idx, clipper) => {
                if let Some(StageConfig::Preamp(cfg)) = self.stages.get_mut(idx) {
                    cfg.clipper_type = clipper;
                }
                should_update_chain = true;
            }

            // Compressor updates
            Message::CompressorThresholdChanged(idx, v) => {
                if let Some(StageConfig::Compressor(cfg)) = self.stages.get_mut(idx) {
                    cfg.threshold_db = v;
                }
                should_update_chain = true;
            }
            Message::CompressorRatioChanged(idx, v) => {
                if let Some(StageConfig::Compressor(cfg)) = self.stages.get_mut(idx) {
                    cfg.ratio = v;
                }
                should_update_chain = true;
            }
            Message::CompressorAttackChanged(idx, v) => {
                if let Some(StageConfig::Compressor(cfg)) = self.stages.get_mut(idx) {
                    cfg.attack_ms = v;
                }
                should_update_chain = true;
            }
            Message::CompressorReleaseChanged(idx, v) => {
                if let Some(StageConfig::Compressor(cfg)) = self.stages.get_mut(idx) {
                    cfg.release_ms = v;
                }
                should_update_chain = true;
            }
            Message::CompressorMakeupChanged(idx, v) => {
                if let Some(StageConfig::Compressor(cfg)) = self.stages.get_mut(idx) {
                    cfg.makeup_db = v;
                }
                should_update_chain = true;
            }

            // ToneStack updates
            Message::ToneStackModelChanged(idx, model) => {
                if let Some(StageConfig::ToneStack(cfg)) = self.stages.get_mut(idx) {
                    cfg.model = model;
                }
                should_update_chain = true;
            }
            Message::ToneStackBassChanged(idx, v) => {
                if let Some(StageConfig::ToneStack(cfg)) = self.stages.get_mut(idx) {
                    cfg.bass = v;
                }
                should_update_chain = true;
            }
            Message::ToneStackMidChanged(idx, v) => {
                if let Some(StageConfig::ToneStack(cfg)) = self.stages.get_mut(idx) {
                    cfg.mid = v;
                }
                should_update_chain = true;
            }
            Message::ToneStackTrebleChanged(idx, v) => {
                if let Some(StageConfig::ToneStack(cfg)) = self.stages.get_mut(idx) {
                    cfg.treble = v;
                }
                should_update_chain = true;
            }
            Message::ToneStackPresenceChanged(idx, v) => {
                if let Some(StageConfig::ToneStack(cfg)) = self.stages.get_mut(idx) {
                    cfg.presence = v;
                }
                should_update_chain = true;
            }

            // PowerAmp updates
            Message::PowerAmpTypeChanged(idx, amp_type) => {
                if let Some(StageConfig::PowerAmp(cfg)) = self.stages.get_mut(idx) {
                    cfg.amp_type = amp_type;
                }
                should_update_chain = true;
            }
            Message::PowerAmpDriveChanged(idx, v) => {
                if let Some(StageConfig::PowerAmp(cfg)) = self.stages.get_mut(idx) {
                    cfg.drive = v;
                }
                should_update_chain = true;
            }
            Message::PowerAmpSagChanged(idx, v) => {
                if let Some(StageConfig::PowerAmp(cfg)) = self.stages.get_mut(idx) {
                    cfg.sag = v;
                }
                should_update_chain = true;
            }
        }

        if should_update_chain {
            self.update_processor_chain();
        }

        Task::none()
    }

    fn update_processor_chain(&self) {
        let sample_rate = self.processor_manager.sample_rate();
        let chain = self.to_amp_chain(sample_rate);
        self.processor_manager.set_amp_chain(chain);
    }

    fn view(&self) -> Element<Message> {
        let mut list = column![].spacing(10).width(Length::Fill);

        // Add all stages to the list
        for (idx, stage) in self.stages.iter().enumerate() {
            let widget = match stage {
                StageConfig::Filter(cfg) => filter::filter_widget(idx, cfg, self.stages.len()),
                StageConfig::Preamp(cfg) => preamp::preamp_widget(idx, cfg, self.stages.len()),
                StageConfig::Compressor(cfg) => {
                    compressor::compressor_widget(idx, cfg, self.stages.len())
                }
                StageConfig::ToneStack(cfg) => {
                    tonestack::tonestack_widget(idx, cfg, self.stages.len())
                }
                StageConfig::PowerAmp(cfg) => {
                    poweramp::poweramp_widget(idx, cfg, self.stages.len())
                }
            };
            list = list.push(widget);
        }

        let scrollable_content = scrollable(list).height(Length::FillPortion(9));

        let stage_types = vec![
            StageType::Filter,
            StageType::Preamp,
            StageType::Compressor,
            StageType::ToneStack,
            StageType::PowerAmp,
        ];

        let footer = row![
            pick_list(
                stage_types,
                Some(self.selected_stage_type),
                Message::StageTypeSelected
            ),
            button("Add Stage").on_press(Message::AddStage),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(column![scrollable_content, footer].spacing(20).padding(20))
            .width(Length::Fill)
            .height(Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn to_amp_chain(&self, sample_rate: f32) -> AmplifierChain {
        let mut chain = AmplifierChain::new("Custom Amp Chain");

        for (idx, stage) in self.stages.iter().enumerate() {
            match stage {
                StageConfig::Filter(cfg) => {
                    chain.add_stage(Box::new(FilterStage::new(
                        &format!("Filter {}", idx),
                        cfg.filter_type,
                        cfg.cutoff_hz,
                        cfg.resonance,
                        sample_rate,
                    )));
                }
                StageConfig::Preamp(cfg) => {
                    chain.add_stage(Box::new(PreampStage::new(
                        &format!("Preamp {}", idx),
                        cfg.gain,
                        cfg.bias,
                        cfg.clipper_type,
                    )));
                }
                StageConfig::Compressor(cfg) => {
                    chain.add_stage(Box::new(CompressorStage::new(
                        &format!("Compressor {}", idx),
                        cfg.attack_ms,
                        cfg.release_ms,
                        cfg.threshold_db,
                        cfg.ratio,
                        cfg.makeup_db,
                        sample_rate,
                    )));
                }
                StageConfig::ToneStack(cfg) => {
                    chain.add_stage(Box::new(ToneStackStage::new(
                        &format!("ToneStack {}", idx),
                        cfg.model,
                        cfg.bass,
                        cfg.mid,
                        cfg.treble,
                        cfg.presence,
                        sample_rate,
                    )));
                }
                StageConfig::PowerAmp(cfg) => {
                    chain.add_stage(Box::new(PowerAmpStage::new(
                        &format!("PowerAmp {}", idx),
                        cfg.drive,
                        cfg.amp_type,
                        cfg.sag,
                        sample_rate,
                    )));
                }
            }
        }

        if !self.stages.is_empty() {
            let stage_indices: Vec<usize> = (0..self.stages.len()).collect();
            chain.define_channel(0, Vec::new(), stage_indices, Vec::new());
            chain.set_channel(0);
        }

        chain
    }
}
