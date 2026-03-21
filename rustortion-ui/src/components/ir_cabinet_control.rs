use iced::widget::{checkbox, column, pick_list, row, slider, text};
use iced::{Alignment, Element, Length};

use crate::components::widgets::common::{
    COLOR_SUBTLE, COLOR_SUCCESS, COLOR_WARNING, SPACING_NORMAL, TEXT_SIZE_INFO, section_container,
    section_title,
};
use crate::messages::Message;
use crate::tr;

pub struct IrCabinetControl {
    available_irs: Vec<String>,
    selected_ir: Option<String>,
    bypassed: bool,
    gain: f32,
}

impl Default for IrCabinetControl {
    fn default() -> Self {
        Self::new(false, 0.1)
    }
}

impl IrCabinetControl {
    pub const fn new(bypassed: bool, gain: f32) -> Self {
        Self {
            available_irs: Vec::new(),
            selected_ir: None,
            bypassed,
            gain,
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

    pub const fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }

    pub const fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn get_selected_ir(&self) -> Option<String> {
        self.selected_ir.clone()
    }

    pub const fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub const fn get_gain(&self) -> f32 {
        self.gain
    }

    pub fn view(&self) -> Element<'static, Message> {
        let ir_selector = row![
            text(tr!(ir)).width(Length::Fixed(80.0)),
            pick_list(
                self.available_irs.clone(),
                self.selected_ir.clone(),
                Message::IrSelected
            )
            .width(Length::Fill),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);

        let bypass_control = checkbox(self.bypassed)
            .label(tr!(bypassed))
            .on_toggle(Message::IrBypassed);

        let gain_label = format!("{}:", tr!(gain));
        let gain_control = row![
            text(gain_label).width(Length::Fixed(80.0)),
            slider(0.0..=1.0, self.gain, Message::IrGainChanged)
                .width(Length::FillPortion(7))
                .step(0.01),
            text(format!("{:.0}%", self.gain * 100.0)).width(Length::FillPortion(2)),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center);

        let status = if self.bypassed {
            let bypassed_status = format!("({})", tr!(bypassed));
            text(bypassed_status)
                .size(TEXT_SIZE_INFO)
                .style(|_| iced::widget::text::Style {
                    color: Some(COLOR_SUBTLE),
                })
        } else if let Some(ref ir_name) = self.selected_ir {
            text(format!("{} {}", tr!(active), ir_name))
                .size(TEXT_SIZE_INFO)
                .style(|_| iced::widget::text::Style {
                    color: Some(COLOR_SUCCESS),
                })
        } else {
            text(tr!(no_ir_loaded))
                .size(TEXT_SIZE_INFO)
                .style(|_| iced::widget::text::Style {
                    color: Some(COLOR_WARNING),
                })
        };

        let content = column![
            section_title(tr!(cabinet_ir)),
            ir_selector,
            gain_control,
            bypass_control,
            status,
        ]
        .spacing(SPACING_NORMAL);

        section_container(content.into())
    }
}
