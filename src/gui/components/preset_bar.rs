use iced::widget::{button, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Task};

use crate::gui::messages::{Message, PresetGuiMessage, PresetMessage};

pub struct PresetBar {
    preset_name_input: String,
    show_save_input: bool,
    show_overwrite_confirmation: bool,
    overwrite_target: String,
}

impl Default for PresetBar {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetBar {
    pub fn new() -> Self {
        Self {
            preset_name_input: String::new(),
            show_save_input: false,
            show_overwrite_confirmation: false,
            overwrite_target: String::new(),
        }
    }

    pub fn handle(&mut self, message: PresetGuiMessage) -> Task<Message> {
        match message {
            PresetGuiMessage::ShowSave => {
                self.show_save_input(true);
            }
            PresetGuiMessage::CancelSave => {
                self.show_save_input(false);
            }
            PresetGuiMessage::NameChanged(name) => {
                self.set_new_preset_name(name);
            }
            PresetGuiMessage::ConfirmOverwrite => {
                self.hide_overwrite_confirmation();
                return Task::done(Message::Preset(PresetMessage::Save(
                    self.preset_name_input.to_owned(),
                )));
            }
            PresetGuiMessage::CancelOverwrite => {
                self.hide_overwrite_confirmation();
            }
        }

        Task::none()
    }

    pub fn set_new_preset_name(&mut self, name: String) {
        self.preset_name_input = name;
    }

    pub fn show_save_input(&mut self, show: bool) {
        self.show_save_input = show;
        if !show {
            self.preset_name_input.clear();
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

    pub fn view(
        &self,
        selected_preset: Option<String>,
        available_presets: Vec<String>,
    ) -> Element<'static, Message> {
        let preset_selector = row![
            text("Preset:").width(Length::Fixed(80.0)),
            pick_list(available_presets.clone(), selected_preset.clone(), |p| {
                PresetMessage::Select(p).into()
            })
            .width(Length::Fixed(200.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        if self.show_overwrite_confirmation {
            let confirmation_controls = row![
                text(format!("Overwrite '{}'?", self.overwrite_target)),
                button("Yes")
                    .on_press(PresetMessage::Gui(PresetGuiMessage::ConfirmOverwrite).into()),
                button("No").on_press(PresetMessage::Gui(PresetGuiMessage::CancelOverwrite).into()),
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
                text_input("Preset name...", &self.preset_name_input)
                    .on_input(|p| PresetMessage::Gui(PresetGuiMessage::NameChanged(p)).into())
                    .width(Length::Fixed(150.0)),
                button("Save")
                    .on_press(PresetMessage::Save(self.preset_name_input.to_owned()).into()),
                button("Cancel").on_press(PresetMessage::Gui(PresetGuiMessage::CancelSave).into()),
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        } else {
            let mut controls = row![
                button("Save As...")
                    .on_press(PresetMessage::Gui(PresetGuiMessage::ShowSave).into()),
            ];

            if let Some(ref preset_name) = selected_preset {
                controls = controls
                    .push(button("Update").on_press(PresetMessage::Update.into()))
                    .push(
                        button("Delete")
                            .on_press(PresetMessage::Delete(preset_name.clone()).into())
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
