use iced::{Element, Length, Task, Theme};
use log::{error, info};

use crate::gui::components::{control::Control, stage_list::StageList};
use crate::gui::config::{StageConfig, StageType};
use crate::gui::messages::{Message, StageMessage};
use crate::io::manager::ProcessorManager;
use crate::sim::chain::AmplifierChain;

#[derive(Debug)]
pub struct AmplifierApp {
    processor_manager: ProcessorManager,
    stages: Vec<StageConfig>,
    selected_stage_type: StageType,
    is_recording: bool,
}

impl AmplifierApp {
    pub fn new(processor_manager: ProcessorManager) -> Self {
        Self {
            processor_manager,
            stages: Vec::new(),
            selected_stage_type: StageType::default(),
            is_recording: false,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let mut should_update_chain = false;

        match message {
            Message::AddStage => {
                let new_stage = StageConfig::create_default(self.selected_stage_type);
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
                if idx > 0 {
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
            Message::StartRecording => match self.processor_manager.enable_recording() {
                Ok(()) => {
                    self.is_recording = true;
                    info!("Recording started");
                }
                Err(e) => {
                    error!("Failed to start recording: {e}");
                }
            },
            Message::StopRecording => {
                self.processor_manager.disable_recording();
                self.is_recording = false;
                info!("Recording stopped");
            }
            Message::Stage(idx, stage_msg) => {
                if self.update_stage(idx, stage_msg) {
                    should_update_chain = true;
                }
            }
        }

        if should_update_chain {
            self.update_processor_chain();
        }

        Task::none()
    }

    fn update_stage(&mut self, idx: usize, msg: StageMessage) -> bool {
        if let Some(stage) = self.stages.get_mut(idx) {
            match (stage, msg) {
                (StageConfig::Filter(cfg), StageMessage::Filter(msg)) => {
                    use crate::gui::messages::FilterMessage::*;
                    match msg {
                        TypeChanged(t) => cfg.filter_type = t,
                        CutoffChanged(v) => cfg.cutoff_hz = v,
                        ResonanceChanged(v) => cfg.resonance = v,
                    }
                    true
                }
                (StageConfig::Preamp(cfg), StageMessage::Preamp(msg)) => {
                    use crate::gui::messages::PreampMessage::*;
                    match msg {
                        GainChanged(v) => cfg.gain = v,
                        BiasChanged(v) => cfg.bias = v,
                        ClipperChanged(c) => cfg.clipper_type = c,
                    }
                    true
                }
                (StageConfig::Compressor(cfg), StageMessage::Compressor(msg)) => {
                    use crate::gui::messages::CompressorMessage::*;
                    match msg {
                        ThresholdChanged(v) => cfg.threshold_db = v,
                        RatioChanged(v) => cfg.ratio = v,
                        AttackChanged(v) => cfg.attack_ms = v,
                        ReleaseChanged(v) => cfg.release_ms = v,
                        MakeupChanged(v) => cfg.makeup_db = v,
                    }
                    true
                }
                (StageConfig::ToneStack(cfg), StageMessage::ToneStack(msg)) => {
                    use crate::gui::messages::ToneStackMessage::*;
                    match msg {
                        ModelChanged(m) => cfg.model = m,
                        BassChanged(v) => cfg.bass = v,
                        MidChanged(v) => cfg.mid = v,
                        TrebleChanged(v) => cfg.treble = v,
                        PresenceChanged(v) => cfg.presence = v,
                    }
                    true
                }
                (StageConfig::PowerAmp(cfg), StageMessage::PowerAmp(msg)) => {
                    use crate::gui::messages::PowerAmpMessage::*;
                    match msg {
                        TypeChanged(t) => cfg.amp_type = t,
                        DriveChanged(v) => cfg.drive = v,
                        SagChanged(v) => cfg.sag = v,
                    }
                    true
                }
                (StageConfig::Level(cfg), StageMessage::Level(msg)) => {
                    use crate::gui::messages::LevelMessage::*;
                    match msg {
                        GainChanged(v) => cfg.gain = v,
                    }
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    fn update_processor_chain(&self) {
        let sample_rate = self.processor_manager.sample_rate();
        let chain = build_amplifier_chain(&self.stages, sample_rate);
        self.processor_manager.set_amp_chain(chain);
    }

    pub fn view(&self) -> Element<Message> {
        use iced::widget::{column, container};

        container(
            column![
                StageList::new(&self.stages).view(),
                Control::new(self.selected_stage_type, self.is_recording).view()
            ]
            .spacing(20)
            .padding(20),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn build_amplifier_chain(stages: &[StageConfig], sample_rate: f32) -> AmplifierChain {
    let mut chain = AmplifierChain::new("Custom Amp Chain");

    for (idx, stage) in stages.iter().enumerate() {
        match stage {
            StageConfig::Filter(cfg) => {
                chain.add_stage(Box::new(
                    cfg.to_stage(&format!("Filter {idx}"), sample_rate),
                ));
            }
            StageConfig::Preamp(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(&format!("Preamp {idx}"))));
            }
            StageConfig::Compressor(cfg) => {
                chain.add_stage(Box::new(
                    cfg.to_stage(&format!("Compressor {idx}"), sample_rate),
                ));
            }
            StageConfig::ToneStack(cfg) => {
                chain.add_stage(Box::new(
                    cfg.to_stage(&format!("ToneStack {idx}"), sample_rate),
                ));
            }
            StageConfig::PowerAmp(cfg) => {
                chain.add_stage(Box::new(
                    cfg.to_stage(&format!("PowerAmp {idx}"), sample_rate),
                ));
            }
            StageConfig::Level(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(&format!("Level {idx}"))));
            }
            StageConfig::Cabinet(cfg) => {
                let stage = cfg.to_stage(&format!("Cabinet {idx}")).expect("Error");
                chain.add_stage(Box::new(stage));
            }
        }
    }

    // Define a simple linear channel with all stages
    if !stages.is_empty() {
        let stage_indices: Vec<usize> = (0..stages.len()).collect();
        chain.define_channel(0, Vec::new(), stage_indices, Vec::new());
        chain.set_channel(0);
    }

    chain
}
