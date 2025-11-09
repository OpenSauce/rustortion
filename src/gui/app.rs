use iced::{Element, Length, Subscription, Task, Theme, time, time::Duration};
use log::{error, info};
use std::path::Path;

use crate::audio::manager::Manager;
use crate::gui::components::ir_cabinet_control::IrCabinetControl;
use crate::gui::components::{
    control::Control, dialogs::settings::SettingsDialog, dialogs::tuner::TunerDisplay,
    preset_bar::PresetBar, stage_list::StageList,
};
use crate::gui::config::{StageConfig, StageType};
use crate::gui::messages::Message;
use crate::gui::preset::{Preset, PresetManager};
use crate::gui::settings::Settings;
use crate::sim::chain::AmplifierChain;

const PRESET_DIR: &str = "./presets";
const REBUILD_INTERVAL: Duration = Duration::from_millis(100);
const TUNER_POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct AmplifierApp {
    audio_manager: Manager,
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
    ir_cabinet_control: IrCabinetControl,
    tuner_dialog: TunerDisplay,
    tuner_enabled: bool,
}

impl AmplifierApp {
    pub fn new(audio_manager: Manager, settings: Settings) -> Self {
        let preset_manager = PresetManager::new(PRESET_DIR).unwrap_or_else(|e| {
            error!("Failed to create preset manager: {e}");

            // Create an empty preset manager as fallback
            PresetManager::new(std::env::temp_dir().join("rustortion_presets_fallback"))
                .expect("Failed to create fallback preset manager")
        });

        let presets = preset_manager.get_presets();
        let selected_preset = presets.first().map(|p| p.name.clone());
        let preset_bar = PresetBar::new(presets, selected_preset.clone());

        let stages = selected_preset
            .as_deref()
            .and_then(|n| preset_manager.get_preset_by_name(n))
            .map(|p| p.stages.clone())
            .unwrap_or_default();

        let stage_list = StageList::new(stages.clone());
        let control_bar = Control::new(StageType::default());
        let settings_dialog = SettingsDialog::new(&settings.audio);

        let mut ir_cabinet_control = IrCabinetControl::new();

        // Load available IRs from the ir/ directory
        if let Ok(irs) = Self::scan_ir_directory() {
            ir_cabinet_control.set_available_irs(irs);

            // Set the first IR as active if available
            if let Some(first_ir) = ir_cabinet_control.get_selected_ir() {
                audio_manager.engine().set_ir_cabinet(Some(first_ir));
            }
        }

        Self {
            audio_manager,
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
            ir_cabinet_control,
            tuner_dialog: TunerDisplay::new(),
            tuner_enabled: false,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row};

        let top_bar = row![
            Space::with_width(Length::Fill),
            button("Tuner")
                .on_press(Message::ToggleTuner)
                .style(iced::widget::button::secondary),
            button("Settings").on_press(Message::OpenSettings),
        ]
        .spacing(5);

        let main_content = column![
            top_bar,
            self.preset_bar.view(),
            self.stage_list.view(),
            self.ir_cabinet_control.view(),
            self.control_bar.view(self.is_recording),
        ]
        .spacing(20)
        .padding(20);

        if let Some(dialog) = self.settings_dialog.view() {
            dialog
        } else if let Some(tuner_dialog) = self.tuner_dialog.view() {
            tuner_dialog
        } else {
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let rebuild_sub = if self.dirty_chain {
            time::every(REBUILD_INTERVAL).map(|_| Message::RebuildTick)
        } else {
            Subscription::none()
        };

        let tuner_sub = if self.tuner_enabled {
            time::every(TUNER_POLL_INTERVAL).map(|_| Message::TunerUpdate)
        } else {
            Subscription::none()
        };

        Subscription::batch(vec![rebuild_sub, tuner_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RebuildTick => self.rebuild_if_dirty(),
            Message::AddStage => {
                let new_stage = StageConfig::from(self.control_bar.selected());
                self.stages.push(new_stage);
                self.mark_stages_dirty();
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                    self.mark_stages_dirty();
                }
            }
            Message::MoveStageUp(idx) => {
                if idx > 0 && idx < self.stages.len() {
                    self.stages.swap(idx - 1, idx);
                    self.mark_stages_dirty();
                }
            }
            Message::MoveStageDown(idx) => {
                if idx + 1 < self.stages.len() {
                    self.stages.swap(idx, idx + 1);
                    self.mark_stages_dirty();
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.control_bar.set_selected_stage_type(stage_type);
            }
            Message::PresetSelected(preset_name) => {
                if self.selected_preset.as_deref() != Some(preset_name.as_str()) {
                    self.load_preset_by_name(&preset_name);
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
                let name = self.new_preset_name.clone();
                if !name.trim().is_empty() {
                    self.save_preset_named(&name);
                }
            }
            Message::UpdateCurrentPreset => {
                if let Some(name) = self.selected_preset.clone() {
                    self.save_preset_named(&name);
                }
            }
            Message::DeletePreset(preset_name) => {
                self.delete_preset(&preset_name);
            }
            Message::ConfirmOverwritePreset => {
                let name = self.new_preset_name.clone();
                if !name.trim().is_empty() {
                    self.save_preset_named(&name);
                }
                self.preset_bar.hide_overwrite_confirmation();
            }
            Message::CancelOverwritePreset => {
                self.preset_bar.hide_overwrite_confirmation();
            }
            Message::StartRecording => {
                let sample_rate = self.audio_manager.sample_rate();
                let recording_dir = &self.settings.recording_dir;
                if let Err(e) = self
                    .audio_manager
                    .engine()
                    .start_recording(sample_rate, recording_dir)
                {
                    error!("Failed to start recording: {}", e);
                } else {
                    self.is_recording = true;
                }
            }
            Message::StopRecording => {
                self.audio_manager.engine().stop_recording();
                self.is_recording = false;
                info!("Recording stopped");
            }

            Message::OpenSettings => {
                let inputs = self.audio_manager.get_available_inputs();
                let outputs = self.audio_manager.get_available_outputs();
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
                if let Err(e) = self.audio_manager.apply_settings(new_audio_settings) {
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
                let inputs = self.audio_manager.get_available_inputs();
                let outputs = self.audio_manager.get_available_outputs();
                self.settings_dialog
                    .show(&self.settings.audio, inputs, outputs);
            }
            Message::InputPortChanged(p) => self.with_temp_settings(|s| s.input_port = p),
            Message::OutputLeftPortChanged(p) => {
                self.with_temp_settings(|s| s.output_left_port = p)
            }
            Message::OutputRightPortChanged(p) => {
                self.with_temp_settings(|s| s.output_right_port = p)
            }
            Message::BufferSizeChanged(x) => self.with_temp_settings(|s| s.buffer_size = x),
            Message::SampleRateChanged(x) => self.with_temp_settings(|s| s.sample_rate = x),
            Message::OversamplingFactorChanged(x) => {
                self.with_temp_settings(|s| s.oversampling_factor = x)
            }
            Message::AutoConnectToggled(b) => self.with_temp_settings(|s| s.auto_connect = b),
            Message::IrSelected(ir_name) => {
                self.ir_cabinet_control
                    .set_selected_ir(Some(ir_name.clone()));
                self.audio_manager.engine().set_ir_cabinet(Some(ir_name));
            }
            Message::IrBypassed(bypassed) => {
                self.ir_cabinet_control.set_bypassed(bypassed);
                self.audio_manager.engine().set_ir_bypass(bypassed);
            }
            Message::IrGainChanged(gain) => {
                self.ir_cabinet_control.set_gain(gain);
                self.audio_manager.engine().set_ir_gain(gain);
            }
            Message::RefreshIrs => {
                if let Ok(irs) = Self::scan_ir_directory() {
                    self.ir_cabinet_control.set_available_irs(irs);
                    // Re-apply current selection
                    if let Some(selected) = self.ir_cabinet_control.get_selected_ir() {
                        self.audio_manager.engine().set_ir_cabinet(Some(selected));
                    }
                }
            }
            Message::Stage(idx, stage_msg) => {
                if let Some(stage) = self.stages.get_mut(idx)
                    && stage.apply(stage_msg)
                {
                    self.mark_stages_dirty();
                }
            }
            Message::ToggleTuner => {
                self.tuner_enabled = !self.tuner_enabled;

                if self.tuner_enabled {
                    self.tuner_dialog.show();
                    self.audio_manager.engine().set_tuner_enabled(true);
                } else {
                    self.tuner_dialog.hide();
                    self.audio_manager.engine().set_tuner_enabled(false);
                }
            }
            Message::TunerUpdate => {
                if self.tuner_enabled {
                    self.tuner_dialog
                        .update(self.audio_manager.tuner().get_tuner_info());
                }
            }
        }

        Task::none()
    }

    fn scan_ir_directory() -> Result<Vec<String>, std::io::Error> {
        use std::fs;
        use std::path::Path;

        let ir_path = Path::new("./impulse_responses");
        if !ir_path.exists() {
            fs::create_dir_all(ir_path)?;
        }

        let mut irs = Vec::new();
        Self::scan_ir_recursive(ir_path, ir_path, &mut irs)?;

        irs.sort_by(|a, b| {
            let a_sep_count = a.matches('/').count();
            let b_sep_count = b.matches('/').count();
            a_sep_count.cmp(&b_sep_count).then_with(|| a.cmp(b))
        });

        Ok(irs)
    }

    fn scan_ir_recursive(
        current_dir: &Path,
        base_dir: &Path,
        irs: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively scan subdirectories
                Self::scan_ir_recursive(&path, base_dir, irs)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                // Get relative path from base_dir
                let relative_path = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"); // Normalize path separators
                irs.push(relative_path);
            }
        }
        Ok(())
    }

    fn rebuild_if_dirty(&mut self) {
        if !self.dirty_chain {
            return;
        }
        self.update_processor_chain();
        self.dirty_chain = false;
    }

    fn mark_stages_dirty(&mut self) {
        self.dirty_chain = true;
        self.stage_list.set_stages(&self.stages);
    }

    fn delete_preset(&mut self, preset_name: &str) {
        if let Err(e) = self.preset_manager.delete_preset(preset_name) {
            error!("Failed to delete preset: {e}");
            return;
        }

        info!("Deleted preset: {preset_name}");

        let presets = self.preset_manager.get_presets();
        self.preset_bar.update_presets(presets);

        if self.selected_preset.as_deref() == Some(preset_name) {
            if let Some(first) = presets.first() {
                self.stages = first.stages.clone();
                self.selected_preset = Some(first.name.clone());
                self.preset_bar
                    .set_selected_preset(self.selected_preset.clone());
            } else {
                self.stages.clear();
                self.selected_preset = None;
                self.preset_bar.set_selected_preset(None);
            }
            self.mark_stages_dirty();
        }
    }

    fn load_preset_by_name(&mut self, name: &str) {
        if let Some(p) = self.preset_manager.get_preset_by_name(name) {
            self.stages = p.stages.clone();
            self.selected_preset = Some(name.to_owned());
            self.preset_bar
                .set_selected_preset(self.selected_preset.clone());
            self.mark_stages_dirty();
            info!("Loaded preset: {name}");
        }
    }

    fn refresh_presets_and_select(&mut self, name: Option<String>) {
        self.preset_bar
            .update_presets(self.preset_manager.get_presets());
        self.selected_preset = name.clone();
        self.preset_bar.set_selected_preset(name);
    }

    fn save_preset_named(&mut self, name: &str) {
        let preset = Preset::new(name.to_owned(), self.stages.clone());
        match self.preset_manager.save_preset(&preset) {
            Ok(()) => {
                info!("Saved preset: {name}");
                self.refresh_presets_and_select(Some(name.to_owned()));
                self.show_save_input = false;
                self.preset_bar.show_save_input(false);
                self.new_preset_name.clear();
            }
            Err(e) => error!("Failed to save preset: {e}"),
        }
    }

    fn update_processor_chain(&self) {
        let sample_rate = self.audio_manager.sample_rate();
        let chain = self.build_amplifier_chain(sample_rate);
        self.audio_manager.engine().set_amp_chain(chain);
    }

    fn with_temp_settings<F: FnOnce(&mut crate::gui::settings::AudioSettings)>(&mut self, f: F) {
        let mut tmp = self.settings_dialog.get_settings();
        f(&mut tmp);
        self.settings_dialog.update_temp_settings(tmp);
    }

    fn build_amplifier_chain(&self, sample_rate: usize) -> AmplifierChain {
        let mut chain = AmplifierChain::new();

        let effective_sample_rate = sample_rate * self.settings.audio.oversampling_factor as usize;

        for cfg in &self.stages {
            chain.add_stage(cfg.to_runtime(effective_sample_rate as f32));
        }

        chain
    }
}
