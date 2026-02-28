use iced::Element;
use iced::keyboard::{Key, Modifiers};
use log::debug;

use crate::gui::components::dialogs::hotkey::HotkeyDialog;
use crate::gui::messages::{HotkeyMessage, Message};
use crate::settings::HotkeySettings;

pub struct HotkeyHandler {
    dialog: HotkeyDialog,
    settings: HotkeySettings,
}

impl HotkeyHandler {
    pub fn new(settings: HotkeySettings) -> Self {
        Self {
            dialog: HotkeyDialog::new(),
            settings,
        }
    }

    /// Handle a hotkey message. Returns `true` if settings were modified and need saving.
    pub fn handle(&mut self, message: HotkeyMessage, presets: Vec<String>) -> bool {
        match message {
            HotkeyMessage::Open => {
                self.dialog.show(presets, self.settings.mappings.clone());
                false
            }
            HotkeyMessage::Close => {
                self.dialog.hide();
                false
            }
            HotkeyMessage::StartLearning => {
                self.dialog.start_learning();
                false
            }
            HotkeyMessage::CancelLearning => {
                self.dialog.cancel_learning();
                false
            }
            HotkeyMessage::PresetSelected(preset) => {
                self.dialog.set_preset_for_mapping(preset);
                false
            }
            HotkeyMessage::ConfirmMapping => {
                if self.dialog.complete_mapping().is_some() {
                    self.settings.mappings = self.dialog.get_mappings();
                    debug!("Hotkey mapping added and saved");
                    true
                } else {
                    false
                }
            }
            HotkeyMessage::RemoveMapping(idx) => {
                self.dialog.remove_mapping(idx);
                self.settings.mappings = self.dialog.get_mappings();
                debug!("Hotkey mapping removed and saved");
                true
            }
        }
    }

    pub fn on_key_input(&mut self, key: &Key, modifiers: Modifiers) {
        self.dialog.on_key_input(key, modifiers);
    }

    pub fn is_learning(&self) -> bool {
        self.dialog.is_learning()
    }

    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    /// Check if a key event matches any hotkey mapping, returning the preset name if so.
    pub fn check_mapping(&self, key: &Key, modifiers: Modifiers) -> Option<String> {
        self.settings
            .mappings
            .iter()
            .find(|m| m.matches(key, modifiers))
            .map(|m| m.preset_name.clone())
    }

    pub fn settings(&self) -> &HotkeySettings {
        &self.settings
    }

    pub fn view(&self) -> Option<Element<'_, Message>> {
        self.dialog.view()
    }
}
