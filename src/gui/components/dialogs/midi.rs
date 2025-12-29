use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Alignment, Color, Element, Length};

use crate::gui::messages::Message;
use crate::midi::{MidiInputEvent, MidiManager, MidiMapping};

const MAX_DEBUG_MESSAGES: usize = 20;

/// State for the "learning" mode where we wait for a MIDI input
#[derive(Debug, Clone, PartialEq)]
pub enum LearningState {
    /// Not learning, normal operation
    Idle,
    /// Waiting for user to select a MIDI input
    WaitingForInput,
    /// Input captured, waiting for preset selection
    InputCaptured {
        channel: u8,
        control: u8,
        description: String,
    },
}

pub struct MidiDialog {
    show_dialog: bool,
    available_controllers: Vec<String>,
    selected_controller: Option<String>,
    mappings: Vec<MidiMapping>,
    available_presets: Vec<String>,
    learning_state: LearningState,
    debug_messages: Vec<String>,
    /// Preset selected for new mapping
    selected_preset_for_mapping: Option<String>,
}

impl Default for MidiDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiDialog {
    pub fn new() -> Self {
        Self {
            show_dialog: false,
            available_controllers: Vec::new(),
            selected_controller: None,
            mappings: Vec::new(),
            available_presets: Vec::new(),
            learning_state: LearningState::Idle,
            debug_messages: Vec::new(),
            selected_preset_for_mapping: None,
        }
    }

    pub fn show(&mut self, presets: Vec<String>, mappings: Vec<MidiMapping>) {
        self.show_dialog = true;
        self.available_presets = presets;
        self.mappings = mappings;
        self.learning_state = LearningState::Idle;
        self.refresh_controllers();
    }

    pub fn hide(&mut self) {
        self.show_dialog = false;
        self.learning_state = LearningState::Idle;
    }

    pub fn is_visible(&self) -> bool {
        self.show_dialog
    }

    pub fn refresh_controllers(&mut self) {
        self.available_controllers = MidiManager::list_devices();
    }

    pub fn set_selected_controller(&mut self, controller: Option<String>) {
        self.selected_controller = controller;
    }

    pub fn get_selected_controller(&self) -> Option<String> {
        self.selected_controller.clone()
    }

    pub fn set_mappings(&mut self, mappings: Vec<MidiMapping>) {
        self.mappings = mappings;
    }

    pub fn get_mappings(&self) -> Vec<MidiMapping> {
        self.mappings.clone()
    }

    pub fn start_learning(&mut self) {
        self.learning_state = LearningState::WaitingForInput;
        self.selected_preset_for_mapping = None;
    }

    pub fn cancel_learning(&mut self) {
        self.learning_state = LearningState::Idle;
        self.selected_preset_for_mapping = None;
    }

    pub fn is_learning(&self) -> bool {
        matches!(
            self.learning_state,
            LearningState::WaitingForInput | LearningState::InputCaptured { .. }
        )
    }

    /// Called when a MIDI input is received
    pub fn on_midi_input(&mut self, event: &MidiInputEvent) {
        // Add to debug log
        let debug_msg = format!("{}", event);
        self.debug_messages.insert(0, debug_msg);
        if self.debug_messages.len() > MAX_DEBUG_MESSAGES {
            self.debug_messages.pop();
        }

        // If we're waiting for input, capture it
        if self.learning_state == LearningState::WaitingForInput {
            self.learning_state = LearningState::InputCaptured {
                channel: event.channel,
                control: event.control,
                description: format!("{}", event),
            };
        }
    }

    /// Set the preset for the new mapping
    pub fn set_preset_for_mapping(&mut self, preset: String) {
        self.selected_preset_for_mapping = Some(preset);
    }

    /// Complete adding a new mapping
    pub fn complete_mapping(&mut self) -> Option<MidiMapping> {
        let LearningState::InputCaptured {
            channel, control, ..
        } = self.learning_state
        else {
            return None;
        };

        let preset_name = self.selected_preset_for_mapping.as_ref()?;

        let mapping = MidiMapping::new(channel, control, preset_name.clone());

        // Remove any existing mapping for the same input
        self.mappings
            .retain(|m| !(m.channel == channel && m.control == control));

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

        let title =
            text("MIDI Settings")
                .size(24)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                });

        // Controller selection section
        let controller_section = self.controller_section_view();

        // Mappings section
        let mappings_section = self.mappings_section_view();

        // Debug section
        let debug_section = self.debug_section_view();

        // Controls
        let controls = row![
            button("Refresh Controllers").on_press(Message::MidiRefreshControllers),
            iced::widget::horizontal_space(),
            button("Close").on_press(Message::MidiClose),
        ]
        .spacing(10)
        .width(Length::Fill);

        let dialog_content = column![
            title,
            iced::widget::rule::Rule::horizontal(1),
            controller_section,
            iced::widget::rule::Rule::horizontal(1),
            mappings_section,
            iced::widget::rule::Rule::horizontal(1),
            debug_section,
            controls,
        ]
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

    fn controller_section_view(&self) -> Element<'_, Message> {
        let header =
            text("Controller")
                .size(18)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                });

        let status_text = if self.selected_controller.is_some() {
            text("Connected")
                .size(14)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.3, 1.0, 0.3)),
                })
        } else {
            text("Not connected")
                .size(14)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
                })
        };

        let controller_picker = row![
            text("Device:").width(Length::Fixed(80.0)),
            pick_list(
                self.available_controllers.clone(),
                self.selected_controller.clone(),
                Message::MidiControllerSelected
            )
            .width(Length::Fill)
            .placeholder("Select a MIDI controller..."),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let disconnect_button = if self.selected_controller.is_some() {
            button("Disconnect")
                .on_press(Message::MidiDisconnect)
                .style(iced::widget::button::danger)
        } else {
            button("Disconnect").style(iced::widget::button::secondary)
        };

        container(
            column![
                row![header, iced::widget::horizontal_space(), status_text]
                    .align_y(Alignment::Center),
                controller_picker,
                disconnect_button,
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

    fn mappings_section_view(&self) -> Element<'_, Message> {
        let header = text("Input Mappings")
            .size(18)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        let add_button = if self.learning_state == LearningState::Idle {
            button("Add Mapping")
                .on_press(Message::MidiStartLearning)
                .style(iced::widget::button::success)
        } else {
            button("Cancel")
                .on_press(Message::MidiCancelLearning)
                .style(iced::widget::button::danger)
        };

        let learning_content: Element<'_, Message> = match &self.learning_state {
            LearningState::Idle => column![].into(),
            LearningState::WaitingForInput => container(
                text("Press a button or move a control on your MIDI device...")
                    .size(16)
                    .style(|_: &iced::Theme| iced::widget::text::Style {
                        color: Some(Color::from_rgb(1.0, 0.8, 0.3)),
                    }),
            )
            .padding(10)
            .style(|_: &iced::Theme| {
                container::Style::default()
                    .background(Color::from_rgba(1.0, 0.8, 0.0, 0.1))
                    .border(iced::Border::default().rounded(5))
            })
            .width(Length::Fill)
            .into(),
            LearningState::InputCaptured { description, .. } => {
                let captured_text =
                    text(format!("Captured: {}", description))
                        .size(16)
                        .style(|_: &iced::Theme| iced::widget::text::Style {
                            color: Some(Color::from_rgb(0.3, 1.0, 0.3)),
                        });

                let preset_picker = row![
                    text("Assign to:").width(Length::Fixed(80.0)),
                    pick_list(
                        self.available_presets.clone(),
                        self.selected_preset_for_mapping.clone(),
                        Message::MidiPresetForMappingSelected
                    )
                    .width(Length::Fill)
                    .placeholder("Select a preset..."),
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                let confirm_button = if self.selected_preset_for_mapping.is_some() {
                    button("Confirm Mapping")
                        .on_press(Message::MidiConfirmMapping)
                        .style(iced::widget::button::success)
                } else {
                    button("Confirm Mapping").style(iced::widget::button::secondary)
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
            text("No mappings configured")
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
                        .on_press(Message::MidiRemoveMapping(idx))
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
                row![header, iced::widget::horizontal_space(), add_button]
                    .align_y(Alignment::Center),
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

    fn debug_section_view(&self) -> Element<'_, Message> {
        let header =
            text("Debug Log")
                .size(18)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                });

        let debug_content: Element<'_, Message> = if self.debug_messages.is_empty() {
            text("No MIDI messages received yet")
                .size(12)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                })
                .into()
        } else {
            let mut col = column![].spacing(2);

            for msg in &self.debug_messages {
                col = col.push(text(msg).size(12).style(|_: &iced::Theme| {
                    iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.7, 0.9, 0.7)),
                    }
                }));
            }

            scrollable(col).height(Length::Fixed(100.0)).into()
        };

        container(column![header, debug_content,].spacing(10).padding(10))
            .style(|_theme: &iced::Theme| {
                container::Style::default()
                    .background(Color::from_rgba(0.0, 0.0, 0.0, 0.3))
                    .border(iced::Border::default().rounded(5))
            })
            .width(Length::Fill)
            .into()
    }
}
