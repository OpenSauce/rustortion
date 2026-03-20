use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use nih_plug::prelude::{Editor, GuiContext};
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::{
    Alignment, Button, Column, Command, Element, IcedEditor, IcedState, Length, Row, Text,
    WindowQueue, alignment, button, create_iced_editor, executor,
};

use crate::RustortionParams;

const fn oversampling_label(idx: u8) -> &'static str {
    match idx {
        0 => "1x",
        2 => "4x",
        3 => "8x",
        4 => "16x",
        _ => "2x",
    }
}

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(400, 300)
}

pub fn create(
    params: Arc<RustortionParams>,
    editor_state: Arc<IcedState>,
    preset_names: Arc<Mutex<Vec<String>>>,
    current_preset_idx: Arc<AtomicUsize>,
    oversampling_idx: Arc<AtomicU8>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<RustortionEditor>(
        editor_state,
        (params, preset_names, current_preset_idx, oversampling_idx),
    )
}

struct RustortionEditor {
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    preset_names: Arc<Mutex<Vec<String>>>,
    current_preset_idx: Arc<AtomicUsize>,
    oversampling_idx: Arc<AtomicU8>,

    output_slider_state: nih_widgets::param_slider::State,
    ir_gain_slider_state: nih_widgets::param_slider::State,
    prev_button_state: button::State,
    next_button_state: button::State,
    os_prev_button_state: button::State,
    os_next_button_state: button::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
    PrevPreset,
    NextPreset,
    PrevOversampling,
    NextOversampling,
}

impl IcedEditor for RustortionEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = (
        Arc<RustortionParams>,
        Arc<Mutex<Vec<String>>>,
        Arc<AtomicUsize>,
        Arc<AtomicU8>,
    );

    fn new(
        (params, preset_names, current_preset_idx, oversampling_idx): Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = Self {
            params,
            context,
            preset_names,
            current_preset_idx,
            oversampling_idx,
            output_slider_state: nih_widgets::param_slider::State::default(),
            ir_gain_slider_state: nih_widgets::param_slider::State::default(),
            prev_button_state: button::State::default(),
            next_button_state: button::State::default(),
            os_prev_button_state: button::State::default(),
            os_next_button_state: button::State::default(),
        };
        (editor, Command::none())
    }

    fn context(&self) -> &dyn GuiContext {
        self.context.as_ref()
    }

    fn update(
        &mut self,
        _window: &mut WindowQueue,
        message: Self::Message,
    ) -> Command<Self::Message> {
        match message {
            Message::ParamUpdate(msg) => self.handle_param_message(msg),
            Message::PrevPreset => {
                let idx = self.current_preset_idx.load(Ordering::Relaxed);
                if idx > 0 {
                    self.current_preset_idx.store(idx - 1, Ordering::Relaxed);
                }
            }
            Message::NextPreset => {
                let idx = self.current_preset_idx.load(Ordering::Relaxed);
                let len = self.preset_names.lock().map_or(0, |n| n.len());
                if idx + 1 < len {
                    self.current_preset_idx.store(idx + 1, Ordering::Relaxed);
                }
            }
            Message::PrevOversampling => {
                let idx = self.oversampling_idx.load(Ordering::Relaxed);
                if idx > 0 {
                    self.oversampling_idx.store(idx - 1, Ordering::Relaxed);
                }
            }
            Message::NextOversampling => {
                let idx = self.oversampling_idx.load(Ordering::Relaxed);
                if idx < 4 {
                    self.oversampling_idx.store(idx + 1, Ordering::Relaxed);
                }
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        let idx = self.current_preset_idx.load(Ordering::Relaxed);
        let preset_name_owned = self
            .preset_names
            .lock()
            .ok()
            .and_then(|n| n.get(idx).cloned());
        let preset_name = preset_name_owned.as_deref().unwrap_or("No Preset");

        Column::new()
            .align_items(Alignment::Center)
            .padding(20)
            .spacing(10)
            .push(
                Text::new("Rustortion")
                    .size(30)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(
                        Button::new(&mut self.prev_button_state, Text::new("<").size(16))
                            .on_press(Message::PrevPreset),
                    )
                    .push(
                        Text::new(preset_name)
                            .size(18)
                            .horizontal_alignment(alignment::Horizontal::Center),
                    )
                    .push(
                        Button::new(&mut self.next_button_state, Text::new(">").size(16))
                            .on_press(Message::NextPreset),
                    ),
            )
            .push(
                Text::new("Output Level")
                    .size(14)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                nih_widgets::ParamSlider::new(
                    &mut self.output_slider_state,
                    &self.params.output_level,
                )
                .map(Message::ParamUpdate),
            )
            .push(
                Text::new("Cabinet Level")
                    .size(14)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                nih_widgets::ParamSlider::new(&mut self.ir_gain_slider_state, &self.params.ir_gain)
                    .map(Message::ParamUpdate),
            )
            .push(
                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(Text::new("Oversampling").size(14))
                    .push(
                        Button::new(&mut self.os_prev_button_state, Text::new("<").size(14))
                            .on_press(Message::PrevOversampling),
                    )
                    .push(
                        Text::new(oversampling_label(
                            self.oversampling_idx.load(Ordering::Relaxed),
                        ))
                        .size(16),
                    )
                    .push(
                        Button::new(&mut self.os_next_button_state, Text::new(">").size(14))
                            .on_press(Message::NextOversampling),
                    ),
            )
            .into()
    }

    fn background_color(&self) -> nih_plug_iced::Color {
        nih_plug_iced::Color {
            r: 0.15,
            g: 0.15,
            b: 0.15,
            a: 1.0,
        }
    }
}
