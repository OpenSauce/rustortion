use iced::{Element, Length, Subscription, Task, Theme, time, time::Duration};
use log::{error, info};

use crate::audio::manager::Manager;
use crate::gui::components::ir_cabinet_control::IrCabinetControl;
use crate::gui::components::peak_meter::PeakMeterDisplay;
use crate::gui::components::{
    control::Control,
    dialogs::settings::{JackStatus, SettingsDialog},
    dialogs::tuner::TunerDisplay,
    stage_list::StageList,
};
use crate::gui::config::{StageConfig, StageType};
use crate::gui::handlers::preset::PresetHandler;
use crate::gui::messages::{Message, PresetMessage};
use crate::settings::{AudioSettings, Settings};
use crate::sim::chain::AmplifierChain;

const REBUILD_INTERVAL: Duration = Duration::from_millis(100);
const TUNER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const PEAK_METER_POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct AmplifierApp {
    audio_manager: Manager,
    stages: Vec<StageConfig>,
    is_recording: bool,
    stage_list: StageList,
    control_bar: Control,
    settings: Settings,
    settings_dialog: SettingsDialog,
    dirty_chain: bool,
    ir_cabinet_control: IrCabinetControl,
    tuner_dialog: TunerDisplay,
    tuner_enabled: bool,
    preset_handler: PresetHandler,
    peak_meter_display: PeakMeterDisplay,
}

impl AmplifierApp {
    pub fn new(audio_manager: Manager, settings: Settings) -> Self {
        let mut preset_handler = PresetHandler::new(&settings.preset_dir).unwrap();

        // Try and load the last opened preset
        if let Some(last_opened_preset) = settings.selected_preset.as_deref() {
            preset_handler.load_preset_by_name(last_opened_preset);
        }

        let preset = preset_handler.get_selected_preset().unwrap_or_default();

        let stage_list = StageList::new(preset.stages.clone());
        let control_bar = Control::new(StageType::default());
        let settings_dialog = SettingsDialog::new(&settings.audio);

        let mut ir_cabinet_control = IrCabinetControl::new(settings.ir_bypassed, preset.ir_gain);
        ir_cabinet_control.set_available_irs(audio_manager.get_available_irs());

        if settings.ir_bypassed {
            audio_manager.engine().set_ir_bypass(true);
        }

        audio_manager.engine().set_ir_gain(preset.ir_gain);

        if let Some(ir_name) = preset.ir_name {
            ir_cabinet_control.set_selected_ir(Some(ir_name.clone()));
            audio_manager.engine().set_ir_cabinet(Some(ir_name));
        } else if let Some(first_ir) = ir_cabinet_control.get_selected_ir() {
            ir_cabinet_control.set_selected_ir(Some(first_ir.clone()));
            audio_manager.engine().set_ir_cabinet(Some(first_ir));
        }

        Self {
            audio_manager,
            stages: preset.stages,
            is_recording: false,
            stage_list,
            control_bar,
            settings,
            settings_dialog,
            // Set dirty chain to true to trigger initial rebuild
            dirty_chain: true,
            ir_cabinet_control,
            tuner_dialog: TunerDisplay::new(),
            tuner_enabled: false,
            preset_handler,
            peak_meter_display: PeakMeterDisplay::new(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row};

        let top_bar = row![
            self.peak_meter_display.view(),
            Space::with_width(Length::Fill),
            button("Tuner")
                .on_press(Message::ToggleTuner)
                .style(iced::widget::button::secondary),
            button("Settings").on_press(Message::OpenSettings),
        ]
        .spacing(5);

        let main_content = column![
            top_bar,
            self.preset_handler.view(),
            self.stage_list.view(),
            self.ir_cabinet_control.view(),
            self.control_bar.view(self.is_recording),
        ]
        .spacing(10)
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

    // subscription handles all the periodic tasks that happen in the UI
    // this is usually polling for updates from the tuner, audio engine etc
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

        let peak_meter_sub =
            time::every(PEAK_METER_POLL_INTERVAL).map(|_| Message::PeakMeterUpdate);

        Subscription::batch(vec![rebuild_sub, tuner_sub, peak_meter_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetStages(stages) => {
                self.stages = stages;
                self.mark_stages_dirty();
            }
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
                    info!("Recording started");
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
                let jack_status = self.get_jack_status();
                self.settings_dialog
                    .show(&self.settings.audio, inputs, outputs, jack_status);
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
                let jack_status = self.get_jack_status();
                self.settings_dialog
                    .show(&self.settings.audio, inputs, outputs, jack_status);
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
            Message::IrSelected(ir_name) => {
                self.ir_cabinet_control
                    .set_selected_ir(Some(ir_name.clone()));
                self.audio_manager.engine().set_ir_cabinet(Some(ir_name));
            }
            Message::IrBypassed(bypassed) => {
                self.ir_cabinet_control.set_bypassed(bypassed);
                self.audio_manager.engine().set_ir_bypass(bypassed);
                self.settings.ir_bypassed = bypassed;
                if let Err(e) = self.settings.save() {
                    error!("Failed to save settings: {e}");
                }
            }
            Message::IrGainChanged(gain) => {
                self.ir_cabinet_control.set_gain(gain);
                self.audio_manager.engine().set_ir_gain(gain);
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
            Message::PeakMeterUpdate => {
                let info = self.audio_manager.peak_meter().get_info();
                self.peak_meter_display.update(info);
            }
            Message::Preset(msg) => {
                match msg.clone() {
                    PresetMessage::Select(name) | PresetMessage::Save(name) => {
                        self.settings.selected_preset = Some(name.clone());

                        if let Err(e) = self.settings.save() {
                            error!("Failed to save settings: {e}");
                        }
                    }
                    PresetMessage::Delete(deleted_name) => {
                        if self.settings.selected_preset == Some(deleted_name) {
                            self.settings.selected_preset = None;
                        }

                        if let Err(e) = self.settings.save() {
                            error!("Failed to save settings: {e}");
                        }
                    }
                    _ => {}
                }

                return self.preset_handler.handle(
                    msg,
                    self.stages.clone(),
                    self.ir_cabinet_control.get_selected_ir(),
                    self.ir_cabinet_control.get_gain(),
                );
            }
        }

        Task::none()
    }

    fn get_jack_status(&self) -> JackStatus {
        JackStatus {
            sample_rate: self.audio_manager.sample_rate(),
            buffer_size: self.audio_manager.buffer_size(),
        }
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

    fn update_processor_chain(&self) {
        let sample_rate = self.audio_manager.sample_rate();
        let chain = self.build_amplifier_chain(sample_rate);
        self.audio_manager.engine().set_amp_chain(chain);
    }

    fn with_temp_settings<F: FnOnce(&mut AudioSettings)>(&mut self, f: F) {
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
