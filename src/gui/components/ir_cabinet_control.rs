use iced::widget::{button, checkbox, column, container, pick_list, row, slider, text};
use iced::{Alignment, Element, Length};

use crate::gui::messages::Message;

pub struct IrCabinetControl {
    available_irs: Vec<String>,
    selected_ir: Option<String>,
    bypassed: bool,
    gain: f32,
}

impl Default for IrCabinetControl {
    fn default() -> Self {
        Self::new()
    }
}

impl IrCabinetControl {
    pub fn new() -> Self {
        Self {
            available_irs: Vec::new(),
            selected_ir: None,
            bypassed: false,
            gain: 0.1,
        }
    }

    pub fn set_available_irs(&mut self, irs: Vec<String>) {
        self.available_irs = irs;
        // Auto-select first IR if none selected
        if self.selected_ir.is_none() && !self.available_irs.is_empty() {
            self.selected_ir = Some(self.available_irs[0].clone());
        }
    }

    pub fn set_selected_ir(&mut self, ir: Option<String>) {
        self.selected_ir = ir;
    }

    pub fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn get_selected_ir(&self) -> Option<String> {
        self.selected_ir.clone()
    }

    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub fn get_gain(&self) -> f32 {
        self.gain
    }

    pub fn view(&self) -> Element<'static, Message> {
        let header =
            text("Cabinet IR")
                .size(18)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                });

        let ir_selector = row![
            text("IR:").width(Length::Fixed(80.0)),
            pick_list(
                self.available_irs.clone(),
                self.selected_ir.clone(),
                Message::IrSelected
            )
            .width(Length::Fill),
            button("Refresh").on_press(Message::RefreshIrs),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let bypass_control = checkbox("Bypass", self.bypassed).on_toggle(Message::IrBypassed);

        let gain_control = row![
            text("Gain:").width(Length::Fixed(80.0)),
            slider(0.0..=1.0, self.gain, Message::IrGainChanged)
                .width(Length::FillPortion(7))
                .step(0.01),
            text(format!("{:.0}%", self.gain * 100.0)).width(Length::FillPortion(2)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let status = if self.bypassed {
            text("(Bypassed)")
                .size(14)
                .style(|_| iced::widget::text::Style {
                    color: Some(iced::Color::from_rgb(0.7, 0.7, 0.7)),
                })
        } else if let Some(ref ir_name) = self.selected_ir {
            text(format!("Active: {}", ir_name))
                .size(14)
                .style(|_| iced::widget::text::Style {
                    color: Some(iced::Color::from_rgb(0.3, 1.0, 0.3)),
                })
        } else {
            text("No IR loaded")
                .size(14)
                .style(|_| iced::widget::text::Style {
                    color: Some(iced::Color::from_rgb(1.0, 0.7, 0.3)),
                })
        };

        let content = column![
            header,
            iced::widget::rule::Rule::horizontal(1),
            ir_selector,
            gain_control,
            bypass_control,
            status,
        ]
        .spacing(10)
        .padding(10);

        container(content)
            .width(Length::Fill)
            .style(|theme: &iced::Theme| {
                container::Style::default()
                    .background(theme.palette().background)
                    .border(iced::Border::default().rounded(5))
            })
            .into()
    }
}
