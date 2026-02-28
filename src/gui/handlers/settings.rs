use iced::{Element, Task};
use log::{debug, error};

use crate::audio::manager::Manager;
use crate::gui::components::dialogs::settings::{JackStatus, SettingsDialog};
use crate::gui::messages::{Message, SettingsMessage};
use crate::i18n;
use crate::settings::{AudioSettings, Settings};

pub struct SettingsHandler {
    dialog: SettingsDialog,
}

impl SettingsHandler {
    pub fn new(audio_settings: &AudioSettings) -> Self {
        Self {
            dialog: SettingsDialog::new(audio_settings),
        }
    }

    pub fn handle(
        &mut self,
        message: SettingsMessage,
        settings: &mut Settings,
        audio_manager: &mut Manager,
    ) -> Task<Message> {
        match message {
            SettingsMessage::Open | SettingsMessage::RefreshPorts => {
                let inputs = audio_manager.get_available_inputs();
                let outputs = audio_manager.get_available_outputs();
                let jack_status = JackStatus {
                    sample_rate: audio_manager.sample_rate(),
                    buffer_size: audio_manager.buffer_size(),
                };
                self.dialog
                    .show(&settings.audio, inputs, outputs, jack_status);
            }
            SettingsMessage::Cancel => {
                self.dialog.hide();
            }
            SettingsMessage::Apply => {
                let new_audio_settings = self.dialog.get_settings();
                settings.audio = new_audio_settings.clone();

                if let Err(e) = audio_manager.apply_settings(new_audio_settings) {
                    error!("Failed to apply audio settings: {e}");
                }

                if let Err(e) = settings.save() {
                    error!("Failed to save settings: {e}");
                }

                self.dialog.hide();
                debug!("Audio settings applied successfully");
            }
            SettingsMessage::InputPortChanged(p) => {
                self.with_temp_settings(|s| s.input_port = p);
            }
            SettingsMessage::OutputLeftPortChanged(p) => {
                self.with_temp_settings(|s| s.output_left_port = p);
            }
            SettingsMessage::OutputRightPortChanged(p) => {
                self.with_temp_settings(|s| s.output_right_port = p);
            }
            SettingsMessage::BufferSizeChanged(x) => {
                self.with_temp_settings(|s| s.buffer_size = x);
            }
            SettingsMessage::SampleRateChanged(x) => {
                self.with_temp_settings(|s| s.sample_rate = x);
            }
            SettingsMessage::OversamplingFactorChanged(x) => {
                self.with_temp_settings(|s| s.oversampling_factor = x);
            }
            SettingsMessage::LanguageChanged(lang) => {
                i18n::set_language(lang);
                settings.language = lang;
                if let Err(e) = settings.save() {
                    error!("Failed to save language settings: {e}");
                }
            }
        }

        Task::none()
    }

    fn with_temp_settings<F: FnOnce(&mut AudioSettings)>(&mut self, f: F) {
        let mut tmp = self.dialog.get_settings();
        f(&mut tmp);
        self.dialog.update_temp_settings(tmp);
    }

    pub fn view(&self) -> Option<Element<'_, Message>> {
        self.dialog.view().map(|e| e.map(Message::Settings))
    }

    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    pub fn get_settings(&self) -> AudioSettings {
        self.dialog.get_settings()
    }
}
