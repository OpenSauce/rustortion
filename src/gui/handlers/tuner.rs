use iced::{Element, Task};

use crate::audio::manager::Manager;
use crate::gui::components::dialogs::tuner::TunerDisplay;
use crate::gui::messages::{Message, TunerMessage};

pub struct TunerHandler {
    dialog: TunerDisplay,
    enabled: bool,
}

impl Default for TunerHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TunerHandler {
    pub fn new() -> Self {
        Self {
            dialog: TunerDisplay::new(),
            enabled: false,
        }
    }

    pub fn handle(&mut self, message: TunerMessage, audio_manager: &Manager) -> Task<Message> {
        match message {
            TunerMessage::Toggle => {
                self.enabled = !self.enabled;

                if self.enabled {
                    self.dialog.show();
                    audio_manager.engine().set_tuner_enabled(true);
                } else {
                    self.dialog.hide();
                    audio_manager.engine().set_tuner_enabled(false);
                }
            }
            TunerMessage::Update => {
                if self.enabled {
                    self.dialog.update(audio_manager.tuner().get_tuner_info());
                }
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Option<Element<'_, Message>> {
        self.dialog.view().map(|e| e.map(Message::Tuner))
    }

    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
