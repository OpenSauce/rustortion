use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, column, container, pick_list, row, rule, scrollable, space, text};
use iced::{Alignment, Color, Element, Length};

use crate::gui::messages::Message;
use crate::hotkey::{HotkeyMapping, is_uncapturable_key, serialize_key, serialize_modifiers};
use crate::tr;

/// State for the "learning" mode where we wait for a key press
#[derive(Debug, Clone, PartialEq)]
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
    pub fn new() -> Self {
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

    pub fn is_visible(&self) -> bool {
        self.show_dialog
    }

    pub fn is_learning(&self) -> bool {
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

    pub fn view(&self) -> Option<Element<'_, Message>> {
        if !self.show_dialog {
            return None;
        }

        let title = text(tr!(hotkey_settings))
            .size(24)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        // Mappings section
        let mappings_section = self.mappings_section_view();

        // Controls
        let controls = row![
            space::horizontal(),
            button(tr!(close)).on_press(Message::HotkeyClose),
        ]
        .spacing(10)
        .width(Length::Fill);

        let dialog_content = column![title, rule::horizontal(1), mappings_section, controls,]
            .spacing(15)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill);

        let dialog = container(dialog_content).style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(10).width(2))
        });

        Some(dialog.into())
    }

    fn mappings_section_view(&self) -> Element<'_, Message> {
        let header =
            text(tr!(hotkeys))
                .size(18)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                });

        let add_button = if self.learning_state == LearningState::Idle {
            button(tr!(add_mapping))
                .on_press(Message::HotkeyStartLearning)
                .style(iced::widget::button::success)
        } else {
            button(tr!(cancel))
                .on_press(Message::HotkeyCancelLearning)
                .style(iced::widget::button::danger)
        };

        let learning_content: Element<'_, Message> = match &self.learning_state {
            LearningState::Idle => column![].into(),
            LearningState::WaitingForInput => {
                container(text(tr!(press_any_key)).size(16).style(|_: &iced::Theme| {
                    iced::widget::text::Style {
                        color: Some(Color::from_rgb(1.0, 0.8, 0.3)),
                    }
                }))
                .padding(10)
                .style(|_: &iced::Theme| {
                    container::Style::default()
                        .background(Color::from_rgba(1.0, 0.8, 0.0, 0.1))
                        .border(iced::Border::default().rounded(5))
                })
                .width(Length::Fill)
                .into()
            }
            LearningState::InputCaptured { description, .. } => {
                let captured_text = text(format!("{} {}", tr!(captured), description))
                    .size(16)
                    .style(|_: &iced::Theme| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.3, 1.0, 0.3)),
                    });

                let preset_picker = row![
                    text(tr!(assign_to)).width(Length::Fixed(80.0)),
                    pick_list(
                        self.available_presets.clone(),
                        self.selected_preset_for_mapping.clone(),
                        Message::HotkeyPresetSelected
                    )
                    .width(Length::Fill)
                    .placeholder(tr!(select_preset)),
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                let confirm_button = if self.selected_preset_for_mapping.is_some() {
                    button(tr!(confirm_mapping))
                        .on_press(Message::HotkeyConfirmMapping)
                        .style(iced::widget::button::success)
                } else {
                    button(tr!(confirm_mapping)).style(iced::widget::button::secondary)
                };

                container(column![captured_text, preset_picker, confirm_button,].spacing(10))
                    .padding(10)
                    .style(|_: &iced::Theme| {
                        container::Style::default()
                            .background(Color::from_rgba(0.0, 1.0, 0.0, 0.05))
                            .border(iced::Border::default().rounded(5))
                    })
                    .width(Length::Fill)
                    .into()
            }
        };

        // Existing mappings list
        let mappings_list: Element<'_, Message> = if self.mappings.is_empty() {
            text(tr!(no_mappings_configured))
                .size(14)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                })
                .into()
        } else {
            let mut col = column![].spacing(5);

            for (idx, mapping) in self.mappings.iter().enumerate() {
                let mapping_row = row![
                    text(&mapping.description)
                        .size(14)
                        .width(Length::Fixed(120.0)),
                    text("→").size(14).width(Length::Fixed(30.0)),
                    text(&mapping.preset_name).size(14).width(Length::Fill),
                    button("×")
                        .on_press(Message::HotkeyRemoveMapping(idx))
                        .style(iced::widget::button::danger)
                        .width(Length::Fixed(30.0)),
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                col = col.push(mapping_row);
            }

            scrollable(col).height(Length::Fixed(120.0)).into()
        };

        container(
            column![
                row![header, space::horizontal(), add_button].align_y(Alignment::Center),
                learning_content,
                mappings_list,
            ]
            .spacing(10)
            .padding(10),
        )
        .style(|_theme: &iced::Theme| {
            container::Style::default()
                .background(Color::from_rgba(0.0, 0.0, 0.0, 0.2))
                .border(iced::Border::default().rounded(5))
        })
        .width(Length::Fill)
        .into()
    }
}
