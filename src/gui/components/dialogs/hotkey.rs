use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, column, row, rule, space};
use iced::{Alignment, Element, Length};

use super::common::{
    dialog_container, dialog_section_container, dialog_title_row, input_captured_view,
    mapping_list_view, waiting_for_input_view,
};
use super::{DIALOG_CONTENT_PADDING, DIALOG_CONTENT_SPACING};
use crate::gui::components::widgets::common::{SPACING_NORMAL, TEXT_SIZE_SECTION_TITLE};
use crate::gui::messages::HotkeyMessage;
use crate::hotkey::{HotkeyMapping, is_uncapturable_key, serialize_key, serialize_modifiers};
use crate::tr;

/// State for the "learning" mode where we wait for a key press
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningState {
    /// Not learning, normal operation
    Idle,
    /// Waiting for user to press a key
    WaitingForInput,
    /// Key captured, waiting for preset selection
    InputCaptured {
        key: String,
        modifiers: Vec<String>,
        description: String,
    },
}

pub struct HotkeyDialog {
    show_dialog: bool,
    mappings: Vec<HotkeyMapping>,
    available_presets: Vec<String>,
    learning_state: LearningState,
    /// Preset selected for new mapping
    selected_preset_for_mapping: Option<String>,
}

impl Default for HotkeyDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyDialog {
    pub const fn new() -> Self {
        Self {
            show_dialog: false,
            mappings: Vec::new(),
            available_presets: Vec::new(),
            learning_state: LearningState::Idle,
            selected_preset_for_mapping: None,
        }
    }

    pub fn show(&mut self, presets: Vec<String>, mappings: Vec<HotkeyMapping>) {
        self.show_dialog = true;
        self.available_presets = presets;
        self.mappings = mappings;
        self.learning_state = LearningState::Idle;
    }

    pub fn hide(&mut self) {
        self.show_dialog = false;
        self.learning_state = LearningState::Idle;
    }

    pub const fn is_visible(&self) -> bool {
        self.show_dialog
    }

    pub const fn is_learning(&self) -> bool {
        matches!(
            self.learning_state,
            LearningState::WaitingForInput | LearningState::InputCaptured { .. }
        )
    }

    pub fn start_learning(&mut self) {
        self.learning_state = LearningState::WaitingForInput;
        self.selected_preset_for_mapping = None;
    }

    pub fn cancel_learning(&mut self) {
        self.learning_state = LearningState::Idle;
        self.selected_preset_for_mapping = None;
    }

    /// Called when a key is pressed while in learning mode
    pub fn on_key_input(&mut self, key: &Key, modifiers: Modifiers) {
        // Ignore modifier-only key presses
        if is_uncapturable_key(key) {
            return;
        }

        if self.learning_state != LearningState::WaitingForInput {
            return;
        }

        let Some(key_str) = serialize_key(key) else {
            return;
        };

        let mod_strs = serialize_modifiers(modifiers);
        let description = if mod_strs.is_empty() {
            key_str.clone()
        } else {
            format!("{}+{}", mod_strs.join("+"), key_str)
        };

        self.learning_state = LearningState::InputCaptured {
            key: key_str,
            modifiers: mod_strs,
            description,
        };
    }

    pub fn set_preset_for_mapping(&mut self, preset: String) {
        self.selected_preset_for_mapping = Some(preset);
    }

    pub fn get_mappings(&self) -> Vec<HotkeyMapping> {
        self.mappings.clone()
    }

    /// Complete adding a new mapping
    pub fn complete_mapping(&mut self) -> Option<HotkeyMapping> {
        let LearningState::InputCaptured {
            ref key,
            ref modifiers,
            ..
        } = self.learning_state
        else {
            return None;
        };

        let preset_name = self.selected_preset_for_mapping.as_ref()?;

        let mapping = HotkeyMapping::new(key.clone(), modifiers.clone(), preset_name.clone());

        // Remove any existing mapping for the same key+modifiers
        let key_match = key.clone();
        let mods_match = modifiers.clone();
        self.mappings
            .retain(|m| !(m.key == key_match && m.modifiers == mods_match));

        self.mappings.push(mapping.clone());
        self.learning_state = LearningState::Idle;
        self.selected_preset_for_mapping = None;

        Some(mapping)
    }

    pub fn remove_mapping(&mut self, index: usize) {
        if index < self.mappings.len() {
            self.mappings.remove(index);
        }
    }

    pub fn view(&self) -> Option<Element<'_, HotkeyMessage>> {
        if !self.show_dialog {
            return None;
        }

        let title_row = dialog_title_row(tr!(hotkey_settings), HotkeyMessage::Close);

        // Mappings section
        let mappings_section = self.mappings_section_view();

        let dialog_content = column![title_row, rule::horizontal(1), mappings_section,]
            .spacing(DIALOG_CONTENT_SPACING)
            .padding(DIALOG_CONTENT_PADDING)
            .width(Length::Fill)
            .height(Length::Fill);

        Some(dialog_container(dialog_content.into()))
    }

    fn mappings_section_view(&self) -> Element<'_, HotkeyMessage> {
        let header = iced::widget::text(tr!(hotkeys))
            .size(TEXT_SIZE_SECTION_TITLE)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        let add_button = if self.learning_state == LearningState::Idle {
            button(tr!(add_mapping))
                .on_press(HotkeyMessage::StartLearning)
                .style(iced::widget::button::success)
        } else {
            button(tr!(cancel))
                .on_press(HotkeyMessage::CancelLearning)
                .style(iced::widget::button::danger)
        };

        let learning_content: Element<'_, HotkeyMessage> = match &self.learning_state {
            LearningState::Idle => column![].into(),
            LearningState::WaitingForInput => waiting_for_input_view(tr!(press_any_key)),
            LearningState::InputCaptured { description, .. } => input_captured_view(
                description,
                &self.available_presets,
                self.selected_preset_for_mapping.clone(),
                HotkeyMessage::PresetSelected,
                HotkeyMessage::ConfirmMapping,
            ),
        };

        // Existing mappings list
        let mappings_list = mapping_list_view(
            self.mappings
                .iter()
                .map(|m| (m.description.clone(), m.preset_name.clone()))
                .collect(),
            tr!(no_mappings_configured),
            HotkeyMessage::RemoveMapping,
        );

        dialog_section_container(
            column![
                row![header, space::horizontal(), add_button].align_y(Alignment::Center),
                learning_content,
                mappings_list,
            ]
            .spacing(SPACING_NORMAL)
            .padding(SPACING_NORMAL)
            .into(),
        )
    }
}
