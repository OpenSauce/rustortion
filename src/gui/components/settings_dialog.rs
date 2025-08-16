// src/gui/components/settings_dialog.rs
use iced::widget::{button, column, container, pick_list, row, text, toggler};
use iced::{Alignment, Element, Length};

use crate::gui::messages::Message;
use crate::gui::settings::AudioSettings;

pub struct SettingsDialog {
    temp_settings: AudioSettings,
    available_inputs: Vec<String>,
    available_outputs: Vec<String>,
    show_dialog: bool,
}

impl SettingsDialog {
    pub fn new(settings: &AudioSettings) -> Self {
        Self {
            temp_settings: settings.clone(),
            available_inputs: Vec::new(),
            available_outputs: Vec::new(),
            show_dialog: false,
        }
    }

    pub fn show(
        &mut self,
        current_settings: &AudioSettings,
        inputs: Vec<String>,
        outputs: Vec<String>,
    ) {
        self.temp_settings = current_settings.clone();
        self.available_inputs = inputs;
        self.available_outputs = outputs;

        // Add the current selections if they're not in the lists
        if !self
            .available_inputs
            .contains(&self.temp_settings.input_port)
        {
            self.available_inputs
                .push(self.temp_settings.input_port.clone());
        }
        if !self
            .available_outputs
            .contains(&self.temp_settings.output_left_port)
        {
            self.available_outputs
                .push(self.temp_settings.output_left_port.clone());
        }
        if !self
            .available_outputs
            .contains(&self.temp_settings.output_right_port)
        {
            self.available_outputs
                .push(self.temp_settings.output_right_port.clone());
        }

        self.show_dialog = true;
    }

    pub fn hide(&mut self) {
        self.show_dialog = false;
    }

    pub fn is_visible(&self) -> bool {
        self.show_dialog
    }

    pub fn get_settings(&self) -> AudioSettings {
        self.temp_settings.clone()
    }

    pub fn update_temp_settings(&mut self, settings: AudioSettings) {
        self.temp_settings = settings;
    }

    pub fn view(&self) -> Option<Element<'static, Message>> {
        if !self.show_dialog {
            return None;
        }

        let title = text("Audio Settings")
            .size(24)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        // Input port selection
        let input_section = column![
            text("Input Port:").size(16),
            pick_list(
                self.available_inputs.clone(),
                Some(self.temp_settings.input_port.clone()),
                Message::InputPortChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        // Output port selections
        let output_left_section = column![
            text("Output Left Port:").size(16),
            pick_list(
                self.available_outputs.clone(),
                Some(self.temp_settings.output_left_port.clone()),
                Message::OutputLeftPortChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        let output_right_section = column![
            text("Output Right Port:").size(16),
            pick_list(
                self.available_outputs.clone(),
                Some(self.temp_settings.output_right_port.clone()),
                Message::OutputRightPortChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        // Buffer size selection
        let buffer_sizes = vec![64u32, 128, 256, 512, 1024, 2048, 4096];
        let buffer_section = column![
            text("Buffer Size:").size(16),
            pick_list(buffer_sizes, Some(self.temp_settings.buffer_size), |size| {
                Message::BufferSizeChanged(size)
            })
            .width(Length::Fill),
        ]
        .spacing(5);

        // Sample rate selection
        let sample_rates = vec![44100u32, 48000, 88200, 96000, 176400, 192000];
        let sample_rate_section = column![
            text("Sample Rate:").size(16),
            pick_list(sample_rates, Some(self.temp_settings.sample_rate), |rate| {
                Message::SampleRateChanged(rate)
            })
            .width(Length::Fill),
        ]
        .spacing(5);

        // Auto-connect toggle
        let auto_connect_section = row![
            text("Auto-connect on startup:").size(16),
            toggler(self.temp_settings.auto_connect).on_toggle(Message::AutoConnectToggled)
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Latency display
        let latency = (self.temp_settings.buffer_size as f32
            / self.temp_settings.sample_rate as f32)
            * 1000.0;
        let latency_text =
            text(format!("Latency: {:.2} ms", latency))
                .size(14)
                .style(|_theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(iced::Color::from_rgb(0.7, 0.7, 0.7)),
                });

        // Control buttons
        let controls = row![
            button("Refresh Ports").on_press(Message::RefreshPorts),
            iced::widget::horizontal_space(),
            button("Cancel").on_press(Message::CancelSettings),
            button("Apply")
                .on_press(Message::ApplySettings)
                .style(iced::widget::button::success),
        ]
        .spacing(10)
        .width(Length::Fill);

        let dialog_content = column![
            title,
            iced::widget::rule::Rule::horizontal(1),
            input_section,
            output_left_section,
            output_right_section,
            buffer_section,
            sample_rate_section,
            auto_connect_section,
            latency_text,
            iced::widget::Space::new(Length::Fill, Length::Fixed(10.0)),
            controls,
        ]
        .spacing(15)
        .padding(20)
        .width(Length::Fixed(500.0));

        // Create a modal overlay
        let dialog = container(dialog_content).style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(10).width(2))
        });

        // Center the dialog
        let centered = container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_theme: &iced::Theme| {
                container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7))
            });

        Some(centered.into())
    }
}
