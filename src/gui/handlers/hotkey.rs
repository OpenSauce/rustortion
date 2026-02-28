use iced::Element;
use iced::Task;
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

    pub fn open(&mut self, presets: Vec<String>) {
        self.dialog.show(presets, self.settings.mappings.clone());
    }

    pub fn handle(&mut self, message: HotkeyMessage) -> Task<Message> {
        match message {
            HotkeyMessage::Open => {}
            HotkeyMessage::Close => {
                self.dialog.hide();
            }
            HotkeyMessage::StartLearning => {
                self.dialog.start_learning();
            }
            HotkeyMessage::CancelLearning => {
                self.dialog.cancel_learning();
            }
            HotkeyMessage::PresetSelected(preset) => {
                self.dialog.set_preset_for_mapping(preset);
            }
            HotkeyMessage::ConfirmMapping => {
                if self.dialog.complete_mapping().is_some() {
                    self.settings.mappings = self.dialog.get_mappings();
                    debug!("Hotkey mapping added and saved");
                }
            }
            HotkeyMessage::RemoveMapping(idx) => {
                self.dialog.remove_mapping(idx);
                self.settings.mappings = self.dialog.get_mappings();
                debug!("Hotkey mapping removed and saved");
            }
        }

        Task::none()
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
        self.dialog.view().map(|e| e.map(Message::Hotkey))
    }
}
