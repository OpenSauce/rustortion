use iced::{Element, Length, Subscription, Task, Theme, time, time::Duration};
use log::{error, info};

use crate::gui::components::{
    control::Control, preset_bar::PresetBar, settings_dialog::SettingsDialog, stage_list::StageList,
};
use crate::gui::config::{StageConfig, StageType};
use crate::gui::messages::{Message, StageMessage};
use crate::gui::preset::{Preset, PresetManager};
use crate::gui::settings::Settings;
use crate::io::manager::ProcessorManager;
use crate::sim::chain::AmplifierChain;

pub struct AmplifierApp {
    processor_manager: ProcessorManager,
    stages: Vec<StageConfig>,
    is_recording: bool,
    preset_manager: PresetManager,
    selected_preset: Option<String>,
    preset_bar: PresetBar,
    stage_list: StageList,
    control_bar: Control,
    new_preset_name: String,
    show_save_input: bool,
    settings: Settings,
    settings_dialog: SettingsDialog,

    dirty_chain: bool,
}

impl AmplifierApp {
    pub fn new(processor_manager: ProcessorManager, settings: Settings) -> Self {
        let preset_manager = PresetManager::new("./presets").unwrap_or_else(|e| {
            error!("Failed to create preset manager: {e}");
            // Create an empty preset manager as fallback
            PresetManager::new(std::env::temp_dir().join("rustortion_presets_fallback"))
                .expect("Failed to create fallback preset manager")
        });

        let presets = preset_manager.get_presets();
        let selected_preset = presets.first().map(|p| p.name.clone());
        let preset_bar = PresetBar::new(presets, selected_preset.clone());

        // Load the first preset if available
        let stages = if let Some(preset_name) = &selected_preset {
            preset_manager
                .get_preset_by_name(preset_name)
                .map(|p| p.stages.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let stage_list = StageList::new(stages.clone());
        let control_bar = Control::new(StageType::default());
        let settings_dialog = SettingsDialog::new(&settings.audio);

        Self {
            processor_manager,
            stages,
            is_recording: false,
            preset_manager,
            selected_preset,
            preset_bar,
            stage_list,
            control_bar,
            new_preset_name: String::new(),
            show_save_input: false,
            settings,
            settings_dialog,

            // Set dirty chain to true to trigger initial rebuild
            dirty_chain: true,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.dirty_chain {
            time::every(Duration::from_millis(100)).map(|_| Message::RebuildTick)
        } else {
            Subscription::none()
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let mut should_update_stages = false;

        match message {
            Message::RebuildTick => {
                println!("Rebuild tick received");
                if self.dirty_chain {
                    self.update_processor_chain();
                    self.dirty_chain = false;
                }
            }
            Message::AddStage => {
                let new_stage = StageConfig::create_default(self.control_bar.selected());
                self.stages.push(new_stage);
                self.dirty_chain = true;
                should_update_stages = true;
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                }
                self.dirty_chain = true;
                should_update_stages = true;
            }
            Message::MoveStageUp(idx) => {
                if idx > 0 {
                    self.stages.swap(idx - 1, idx);
                    self.dirty_chain = true;
                    should_update_stages = true;
                }
            }
            Message::MoveStageDown(idx) => {
                if idx < self.stages.len().saturating_sub(1) {
                    self.stages.swap(idx, idx + 1);
                    self.dirty_chain = true;
                    should_update_stages = true;
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.control_bar.set_selected_stage_type(stage_type);
            }
            Message::PresetSelected(preset_name) => {
                if let Some(preset) = self.preset_manager.get_preset_by_name(&preset_name) {
                    self.stages = preset.stages.clone();
                    self.selected_preset = Some(preset_name.clone());
                    self.preset_bar
                        .set_selected_preset(Some(preset_name.clone()));
                    self.dirty_chain = true;
                    should_update_stages = true;
                    info!("Loaded preset: {preset_name}");
                }
            }
            Message::ShowSavePreset => {
                self.show_save_input = true;
                self.preset_bar.show_save_input(true);
            }
            Message::CancelSavePreset => {
                self.show_save_input = false;
                self.new_preset_name.clear();
                self.preset_bar.show_save_input(false);
            }
            Message::PresetNameChanged(name) => {
                self.new_preset_name = name.clone();
                self.preset_bar.set_new_preset_name(name);
            }
            Message::SavePreset => {
                if !self.new_preset_name.trim().is_empty() {
                    let preset = Preset::new(self.new_preset_name.clone(), self.stages.clone());

                    match self.preset_manager.save_preset(&preset) {
                        Ok(()) => {
                            info!("Saved preset: {}", self.new_preset_name);
                            self.selected_preset = Some(self.new_preset_name.clone());
                            self.preset_bar
                                .update_presets(self.preset_manager.get_presets());
                            self.preset_bar
                                .set_selected_preset(Some(self.new_preset_name.clone()));
                            self.show_save_input = false;
                            self.preset_bar.show_save_input(false);
                            self.new_preset_name.clear();
                        }
                        Err(e) => {
                            error!("Failed to save preset: {e}");
                        }
                    }
                }
            }
            Message::UpdateCurrentPreset => {
                if let Some(ref preset_name) = self.selected_preset {
                    let preset = Preset::new(preset_name.clone(), self.stages.clone());

                    match self.preset_manager.save_preset(&preset) {
                        Ok(()) => {
                            info!("Updated preset: {preset_name}");
                            self.preset_bar
                                .update_presets(self.preset_manager.get_presets());
                        }
                        Err(e) => {
                            error!("Failed to update preset: {e}");
                        }
                    }
                }
            }
            Message::DeletePreset(preset_name) => {
                match self.preset_manager.delete_preset(&preset_name) {
                    Ok(()) => {
                        info!("Deleted preset: {preset_name}");
                        self.preset_bar
                            .update_presets(self.preset_manager.get_presets());

                        // Clear selection if we deleted the current preset
                        if self.selected_preset.as_ref() == Some(&preset_name) {
                            self.selected_preset = None;
                            self.preset_bar.set_selected_preset(None);
                            // Optionally load the first available preset or clear stages
                            if let Some(first_preset) = self.preset_manager.get_presets().first() {
                                self.stages = first_preset.stages.clone();
                                self.selected_preset = Some(first_preset.name.clone());
                                self.preset_bar
                                    .set_selected_preset(Some(first_preset.name.clone()));
                            } else {
                                self.stages.clear();
                            }

                            self.dirty_chain = true;
                        }
                    }
                    Err(e) => {
                        error!("Failed to delete preset: {e}");
                    }
                }
            }
            Message::ConfirmOverwritePreset => {
                self.save_current_preset();
                self.preset_bar.hide_overwrite_confirmation();
            }
            Message::CancelOverwritePreset => {
                self.preset_bar.hide_overwrite_confirmation();
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

            Message::OpenSettings => {
                let inputs = self.processor_manager.get_available_inputs();
                let outputs = self.processor_manager.get_available_outputs();
                self.settings_dialog
                    .show(&self.settings.audio, inputs, outputs);
            }
            Message::CancelSettings => {
                self.settings_dialog.hide();
            }
            Message::ApplySettings => {
                let new_audio_settings = self.settings_dialog.get_settings();
                self.settings.audio = new_audio_settings.clone();

                // Apply to processor manager
                if let Err(e) = self.processor_manager.apply_settings(new_audio_settings) {
                    error!("Failed to apply audio settings: {e}");
                }

                // Save settings
                if let Err(e) = self.settings.save() {
                    error!("Failed to save settings: {e}");
                }

                self.settings_dialog.hide();
                info!("Audio settings applied successfully");
            }
            Message::RefreshPorts => {
                let inputs = self.processor_manager.get_available_inputs();
                let outputs = self.processor_manager.get_available_outputs();
                self.settings_dialog
                    .show(&self.settings.audio, inputs, outputs);
            }
            Message::InputPortChanged(port) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.input_port = port;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::OutputLeftPortChanged(port) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.output_left_port = port;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::OutputRightPortChanged(port) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.output_right_port = port;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::BufferSizeChanged(size) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.buffer_size = size;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::SampleRateChanged(rate) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.sample_rate = rate;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::AutoConnectToggled(enabled) => {
                let mut temp_settings = self.settings_dialog.get_settings();
                temp_settings.auto_connect = enabled;
                self.settings_dialog.update_temp_settings(temp_settings);
            }
            Message::Stage(idx, stage_msg) => {
                if self.update_stage(idx, stage_msg) {
                    self.dirty_chain = true;
                    should_update_stages = true
                }
            }
        }

        if should_update_stages {
            self.stage_list.set_stages(&self.stages);
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
                (StageConfig::NoiseGate(cfg), StageMessage::NoiseGate(msg)) => {
                    use crate::gui::messages::NoiseGateMessage::*;
                    match msg {
                        ThresholdChanged(v) => cfg.threshold_db = v,
                        RatioChanged(v) => cfg.ratio = v,
                        AttackChanged(v) => cfg.attack_ms = v,
                        HoldChanged(v) => cfg.hold_ms = v,
                        ReleaseChanged(v) => cfg.release_ms = v,
                    }
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    fn save_current_preset(&mut self) {
        let preset = Preset::new(self.new_preset_name.clone(), self.stages.clone());

        match self.preset_manager.save_preset(&preset) {
            Ok(()) => {
                info!("Saved preset: {}", self.new_preset_name);
                self.selected_preset = Some(self.new_preset_name.clone());
                self.preset_bar
                    .update_presets(self.preset_manager.get_presets());
                self.preset_bar
                    .set_selected_preset(Some(self.new_preset_name.clone()));
                self.show_save_input = false;
                self.preset_bar.show_save_input(false);
                self.new_preset_name.clear();
            }
            Err(e) => {
                error!("Failed to save preset: {e}");
            }
        }
    }
    fn update_processor_chain(&self) {
        let sample_rate = self.processor_manager.sample_rate();
        let chain = build_amplifier_chain(&self.stages, sample_rate);
        self.processor_manager.set_amp_chain(chain);
    }

    pub fn view(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row};

        // Main content with settings button in top bar
        let top_bar = row![
            Space::new(Length::Fill, Length::Fixed(0.0)),
            button("Settings").on_press(Message::OpenSettings)
        ];

        let main_content = column![
            top_bar,
            self.preset_bar.view(),
            self.stage_list.view(),
            self.control_bar.view(self.is_recording),
        ]
        .spacing(20)
        .padding(20);

        // If settings dialog is open, show it as an overlay
        if let Some(dialog) = self.settings_dialog.view() {
            // The dialog already includes a semi-transparent background
            // that covers the entire window, so we just return it
            dialog
        } else {
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn build_amplifier_chain(stages: &[StageConfig], sample_rate: f32) -> AmplifierChain {
    let mut chain = AmplifierChain::new();

    for stage in stages.iter() {
        match stage {
            StageConfig::Filter(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(sample_rate)));
            }
            StageConfig::Preamp(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage()));
            }
            StageConfig::Compressor(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(sample_rate)));
            }
            StageConfig::ToneStack(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(sample_rate)));
            }
            StageConfig::PowerAmp(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(sample_rate)));
            }
            StageConfig::Level(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage()));
            }
            StageConfig::NoiseGate(cfg) => {
                chain.add_stage(Box::new(cfg.to_stage(sample_rate)));
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
