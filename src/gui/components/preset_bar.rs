use iced::widget::{button, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::gui::messages::Message;
use crate::gui::preset::Preset;

pub struct PresetBar {
    available_presets: Vec<String>,
    selected_preset: Option<String>,
    new_preset_name: String,
    show_save_input: bool,
    show_overwrite_confirmation: bool,
    overwrite_target: String,
}

impl PresetBar {
    pub fn new(presets: &[Preset], selected_preset: Option<String>) -> Self {
        let available_presets = presets.iter().map(|p| p.name.clone()).collect();

        Self {
            available_presets,
            selected_preset,
            new_preset_name: String::new(),
            show_save_input: false,
            show_overwrite_confirmation: false,
            overwrite_target: String::new(),
        }
    }

    pub fn update_presets(&mut self, presets: &[Preset]) {
        self.available_presets = presets.iter().map(|p| p.name.clone()).collect();
    }

    pub fn set_selected_preset(&mut self, preset_name: Option<String>) {
        self.selected_preset = preset_name;
    }

    pub fn set_new_preset_name(&mut self, name: String) {
        self.new_preset_name = name;
    }

    pub fn show_save_input(&mut self, show: bool) {
        self.show_save_input = show;
        if !show {
            self.new_preset_name.clear();
            self.show_overwrite_confirmation = false;
            self.overwrite_target.clear();
        }
    }

    pub fn show_overwrite_confirmation(&mut self, preset_name: String) {
        self.show_overwrite_confirmation = true;
        self.overwrite_target = preset_name;
    }

    pub fn hide_overwrite_confirmation(&mut self) {
        self.show_overwrite_confirmation = false;
        self.overwrite_target.clear();
    }

    pub fn view(&self) -> Element<'static, Message> {
        let preset_selector = row![
            text("Preset:").width(Length::Fixed(80.0)),
            pick_list(
                self.available_presets.clone(),
                self.selected_preset.clone(),
                Message::PresetSelected
            )
            .width(Length::Fixed(200.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Show overwrite confirmation dialog
        if self.show_overwrite_confirmation {
            let confirmation_controls = row![
                text(format!("Overwrite '{}'?", self.overwrite_target)),
                button("Yes").on_press(Message::ConfirmOverwritePreset),
                button("No").on_press(Message::CancelOverwritePreset),
            ]
            .spacing(5)
            .align_y(Alignment::Center);

            return container(
                row![
                    preset_selector,
                    iced::widget::horizontal_space(),
                    confirmation_controls,
                ]
                .spacing(20)
                .align_y(Alignment::Center)
                .width(Length::Fill),
            )
            .padding(10)
            .style(|theme: &iced::Theme| {
                container::Style::default()
                    .background(theme.palette().background)
                    .border(iced::Border::default().rounded(5))
            })
            .into();
        }

        let save_controls = if self.show_save_input {
            row![
                text_input("Preset name...", &self.new_preset_name)
                    .on_input(Message::PresetNameChanged)
                    .width(Length::Fixed(150.0)),
                button("Save").on_press(Message::SavePreset),
                button("Cancel").on_press(Message::CancelSavePreset),
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        } else {
            let mut controls = row![button("Save As...").on_press(Message::ShowSavePreset),];

            // Add Update and Delete buttons if a preset is selected
            if let Some(ref preset_name) = self.selected_preset {
                controls = controls
                    .push(button("Update").on_press(Message::UpdateCurrentPreset))
                    .push(
                        button("Delete")
                            .on_press(Message::DeletePreset(preset_name.clone()))
                            .style(iced::widget::button::danger),
                    );
            }

            controls.spacing(5).align_y(Alignment::Center)
        };

        container(
            row![
                preset_selector,
                iced::widget::horizontal_space(),
                save_controls,
            ]
            .spacing(20)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .padding(10)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
    }
}
