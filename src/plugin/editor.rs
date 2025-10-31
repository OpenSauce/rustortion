use nih_plug::prelude::*;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::gui::config::StageConfig;

use super::params::RustortionParams;

pub(crate) fn default_state() -> Arc<IcedState> {
    IcedState::from_size(800, 600)
}

pub(crate) fn create(
    params: Arc<RustortionParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<RustortionEditor>(editor_state, params)
}

struct RustortionEditor {
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    stages: Vec<StageConfig>,

    // Button states (older Iced requires this)
    add_button_state: button::State,
    remove_button_states: Vec<button::State>,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    /// Add a stage
    AddStage,
    /// Remove a stage
    RemoveStage(usize),
}

impl IcedEditor for RustortionEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<RustortionParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        // Load stages from params
        let stages = if let Some(json) = params.stages_json.lock().as_ref() {
            serde_json::from_str(json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let remove_button_states = vec![button::State::default(); stages.len()];

        let editor = RustortionEditor {
            params,
            context,
            stages,
            add_button_state: Default::default(),
            remove_button_states,
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
            Message::AddStage => {
                // For now, just add a default filter stage
                self.stages.push(StageConfig::Filter(Default::default()));
                self.remove_button_states.push(button::State::default());
                self.sync_stages();
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                    self.remove_button_states.remove(idx);
                    self.sync_stages();
                }
            }
        }

        Command::none()
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        let mut content = Column::new()
            .align_items(Alignment::Center)
            .spacing(10)
            .padding(20)
            .push(
                Text::new("Rustortion")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(Space::with_height(10.into()))
            .push(
                Text::new(format!(
                    "Output Gain: {:.1} dB",
                    util::gain_to_db(self.params.output_gain.value())
                ))
                .size(16),
            )
            .push(Space::with_height(20.into()))
            .push(
                Text::new("Stages:")
                    .size(18)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Left),
            );

        // Add stage list - iterate over both stages and button states together
        let stage_rows = self
            .stages
            .iter()
            .zip(self.remove_button_states.iter_mut())
            .enumerate()
            .map(|(idx, (stage, button_state))| {
                let stage_name = match stage {
                    StageConfig::Filter(_) => "Filter",
                    StageConfig::Preamp(_) => "Preamp",
                    StageConfig::Compressor(_) => "Compressor",
                    StageConfig::ToneStack(_) => "Tone Stack",
                    StageConfig::PowerAmp(_) => "Power Amp",
                    StageConfig::Level(_) => "Level",
                    StageConfig::NoiseGate(_) => "Noise Gate",
                };

                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(Text::new(format!("{}. {}", idx + 1, stage_name)).width(Length::Fill))
                    .push(
                        Button::new(button_state, Text::new("Remove"))
                            .on_press(Message::RemoveStage(idx)),
                    )
            });

        // Add all stage rows to content
        for row in stage_rows {
            content = content.push(row);
        }

        // Add button
        content = content.push(
            Button::new(&mut self.add_button_state, Text::new("Add Filter Stage"))
                .on_press(Message::AddStage),
        );

        content.into()
    }
}

impl RustortionEditor {
    fn sync_stages(&self) {
        if let Ok(json) = serde_json::to_string(&self.stages) {
            *self.params.stages_json.lock() = Some(json);
            self.params
                .stages_changed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
