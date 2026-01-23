use iced::widget::{button, column, container, pick_list, row, rule, space, text};
use iced::{Alignment, Color, Element, Length};

use crate::gui::messages::Message;
use crate::i18n::{self, LANGUAGES};
use crate::settings::AudioSettings;
use crate::tr;

const GREEN: Color = Color::from_rgb(0.3, 1.0, 0.3);
const ORANGE: Color = Color::from_rgb(1.0, 0.5, 0.3);

/// Actual JACK settings as reported by the server
#[derive(Debug, Clone, Default)]
pub struct JackStatus {
    pub sample_rate: usize,
    pub buffer_size: usize,
}

/// User Settings
pub struct SettingsDialog {
    temp_settings: AudioSettings,
    available_inputs: Vec<String>,
    available_outputs: Vec<String>,
    show_dialog: bool,
    jack_status: JackStatus,
}

impl SettingsDialog {
    pub fn new(settings: &AudioSettings) -> Self {
        Self {
            temp_settings: settings.clone(),
            available_inputs: Vec::new(),
            available_outputs: Vec::new(),
            show_dialog: false,
            jack_status: JackStatus::default(),
        }
    }

    pub fn show(
        &mut self,
        current_settings: &AudioSettings,
        inputs: Vec<String>,
        outputs: Vec<String>,
        jack_status: JackStatus,
    ) {
        self.temp_settings = current_settings.clone();
        self.available_inputs = inputs;
        self.available_outputs = outputs;
        self.jack_status = jack_status;

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

        let title = text(tr!(audio_settings))
            .size(24)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        // JACK Status section - show actual values from JACK server
        let jack_status_section = self.jack_status_view();

        // Language selection
        let language_section = column![
            text(tr!(language)).size(16),
            pick_list(
                LANGUAGES,
                Some(i18n::get_language()),
                Message::LanguageChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        // Input port selection
        let input_section = column![
            text(tr!(input_port)).size(16),
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
            text(tr!(output_left_port)).size(16),
            pick_list(
                self.available_outputs.clone(),
                Some(self.temp_settings.output_left_port.clone()),
                Message::OutputLeftPortChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        let output_right_section = column![
            text(tr!(output_right_port)).size(16),
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
            text(tr!(buffer_size_requested)).size(16),
            pick_list(
                buffer_sizes,
                Some(self.temp_settings.buffer_size),
                Message::BufferSizeChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        // Sample rate selection
        let sample_rates = vec![44100u32, 48000, 88200, 96000, 176400, 192000];
        let sample_rate_section = column![
            text(tr!(sample_rate_requested)).size(16),
            pick_list(
                sample_rates,
                Some(self.temp_settings.sample_rate),
                Message::SampleRateChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        let oversampling_factors = vec![1u32, 2, 4, 8, 16];
        let oversampling_section = column![
            text(tr!(oversampling_factor)).size(16),
            pick_list(
                oversampling_factors,
                Some(self.temp_settings.oversampling_factor),
                Message::OversamplingFactorChanged
            )
            .width(Length::Fill),
        ]
        .spacing(5);

        // Latency display (based on actual JACK values)
        let latency =
            (self.jack_status.buffer_size as f32 / self.jack_status.sample_rate as f32) * 1000.0;
        let latency_text = text(format!("{} {:.2} {}", tr!(actual_latency), latency, tr!(ms)))
            .size(14)
            .style(|_theme: &iced::Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
            });

        // Control buttons
        let controls = row![
            button(tr!(refresh_ports)).on_press(Message::RefreshPorts),
            space::horizontal(),
            button(tr!(cancel)).on_press(Message::CancelSettings),
            button(tr!(apply))
                .on_press(Message::ApplySettings)
                .style(iced::widget::button::success),
        ]
        .spacing(10)
        .width(Length::Fill);

        let dialog_content = column![
            title,
            rule::horizontal(1),
            jack_status_section,
            rule::horizontal(1),
            row![
                column![
                    language_section,
                    input_section,
                    output_left_section,
                    output_right_section,
                ]
                .spacing(10)
                .padding(5),
                column![
                    buffer_section,
                    sample_rate_section,
                    oversampling_section,
                    latency_text,
                    text(tr!(changes_require_restart))
                        .size(12)
                        .style(|_: &iced::Theme| iced::widget::text::Style {
                            color: Some(Color::from_rgb(1.0, 0.7, 0.3)),
                        }),
                ]
                .spacing(10)
                .padding(5),
            ]
            .spacing(10)
            .padding(5),
            controls,
        ]
        .spacing(15)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill);

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
                container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.7))
            });

        Some(centered.into())
    }

    /// The view containing JACK server status information
    fn jack_status_view(&self) -> Element<'static, Message> {
        let header = text(tr!(jack_server_status))
            .size(18)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        // Check for mismatches between requested and actual
        let sample_rate_match =
            self.jack_status.sample_rate == self.temp_settings.sample_rate as usize;
        let buffer_size_match =
            self.jack_status.buffer_size == self.temp_settings.buffer_size as usize;

        let sample_rate_color = if sample_rate_match { GREEN } else { ORANGE };

        let buffer_size_color = if buffer_size_match { GREEN } else { ORANGE };

        let sample_rate_text = if sample_rate_match {
            format!("{} {}", self.jack_status.sample_rate, tr!(hz))
        } else {
            format!(
                "{} {} ({} {})",
                self.jack_status.sample_rate, tr!(hz), tr!(requested), self.temp_settings.sample_rate
            )
        };

        let buffer_size_text = if buffer_size_match {
            format!("{} {}", self.jack_status.buffer_size, tr!(samples))
        } else {
            format!(
                "{} {} ({} {})",
                self.jack_status.buffer_size, tr!(samples), tr!(requested), self.temp_settings.buffer_size
            )
        };

        let sample_rate_row = row![
            text(tr!(sample_rate)).width(Length::Fixed(120.0)),
            text(sample_rate_text).style(move |_: &iced::Theme| iced::widget::text::Style {
                color: Some(sample_rate_color),
            }),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let buffer_size_row = row![
            text(tr!(buffer_size)).width(Length::Fixed(120.0)),
            text(buffer_size_text).style(move |_: &iced::Theme| iced::widget::text::Style {
                color: Some(buffer_size_color),
            }),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let warning = if !sample_rate_match || !buffer_size_match {
            text(tr!(jack_different_settings))
                .size(12)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(1.0, 0.7, 0.3)),
                })
        } else {
            text("")
        };

        container(
            column![header, sample_rate_row, buffer_size_row, warning,]
                .spacing(8)
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
