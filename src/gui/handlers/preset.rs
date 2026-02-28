use anyhow::Result;
use iced::Element;
use iced::Task;
use log::{debug, error};

use crate::gui::Message;
use crate::gui::components::preset_bar::PresetBar;
use crate::gui::stages::StageConfig;
use crate::preset::{Manager, Preset};

pub struct PresetHandler {
    available_presets: Vec<String>,
    preset_manager: Manager,
    selected_preset: Option<String>,
    preset_bar: PresetBar,
}

impl PresetHandler {
    pub fn new(preset_dir: &str) -> Result<Self> {
        let preset_manager = Manager::new(preset_dir)?;

        let presets = preset_names(&preset_manager);
        let selected_preset = presets.first().cloned();
        let preset_bar = PresetBar::new();

        Ok(Self {
            available_presets: presets,
            preset_manager,
            selected_preset,
            preset_bar,
        })
    }

    pub fn handle(
        &mut self,
        message: crate::gui::messages::PresetMessage,
        stages: Vec<StageConfig>,
        ir_name: Option<String>,
        ir_gain: f32,
        pitch_shift_semitones: i32,
    ) -> Task<Message> {
        use crate::gui::messages::PresetMessage;

        match message {
            PresetMessage::Gui(msg) => return self.preset_bar.handle(msg),
            PresetMessage::Select(preset_name) => {
                if self.selected_preset.as_deref() != Some(preset_name.as_str()) {
                    self.load_preset_by_name(&preset_name);

                    if let Some(preset) = self.get_selected_preset() {
                        return build_preset_load_tasks(preset);
                    }
                }
            }
            PresetMessage::Save(name) => {
                debug!("Saving preset... {name}");
                if !name.trim().is_empty() {
                    self.save_preset_named(&name, stages, ir_name, ir_gain, pitch_shift_semitones);
                }
            }
            PresetMessage::Update => {
                if let Some(name) = self.selected_preset.clone() {
                    self.save_preset_named(&name, stages, ir_name, ir_gain, pitch_shift_semitones);
                }
            }
            PresetMessage::Delete(preset_name) => {
                self.delete_preset(&preset_name);
                if let Some(preset) = self.get_selected_preset() {
                    return build_preset_load_tasks(preset);
                }

                return Task::done(Message::SetStages(Vec::new()));
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'static, Message> {
        self.preset_bar
            .view(self.selected_preset.clone(), self.available_presets.clone())
    }

    pub fn get_selected_preset(&self) -> Option<Preset> {
        self.selected_preset
            .as_ref()
            .and_then(|name| self.preset_manager.get_preset_by_name(name))
            .cloned()
    }

    pub fn get_available_presets(&self) -> Vec<String> {
        self.available_presets.clone()
    }

    pub fn load_preset_by_name(&mut self, name: &str) {
        if self.preset_manager.get_preset_by_name(name).is_some() {
            self.selected_preset = Some(name.to_owned());
            debug!("Loaded preset: {name}");
        }
    }

    fn delete_preset(&mut self, preset_name: &str) {
        if let Err(e) = self.preset_manager.delete_preset(preset_name) {
            error!("Failed to delete preset: {e}");
            return;
        }

        debug!("Deleted preset: {preset_name}");

        self.available_presets = preset_names(&self.preset_manager);

        if self.selected_preset.as_deref() == Some(preset_name) {
            if let Some(first) = self.available_presets.first() {
                self.selected_preset = Some(first.clone());
            } else {
                self.selected_preset = None;
            }
        }
    }

    fn save_preset_named(
        &mut self,
        name: &str,
        stages: Vec<StageConfig>,
        ir_name: Option<String>,
        ir_gain: f32,
        pitch_shift_semitones: i32,
    ) {
        let preset = Preset::new(
            name.to_owned(),
            stages.clone(),
            ir_name,
            ir_gain,
            pitch_shift_semitones,
        );
        match self.preset_manager.save_preset(&preset) {
            Ok(()) => {
                debug!("Saved preset: {name}");
                self.selected_preset = Some(name.to_owned());
                self.preset_bar.show_save_input(false);

                self.available_presets = preset_names(&self.preset_manager);
            }
            Err(e) => error!("Failed to save preset: {e}"),
        }
    }
}

fn preset_names(manager: &Manager) -> Vec<String> {
    manager
        .get_presets()
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

fn build_preset_load_tasks(preset: Preset) -> Task<Message> {
    let set_stage_task = Task::done(Message::SetStages(preset.stages));
    let set_ir_task = match preset.ir_name {
        Some(ir_name) => Task::done(Message::IrSelected(ir_name)),
        None => Task::none(),
    };
    let set_ir_gain_task = Task::done(Message::IrGainChanged(preset.ir_gain));
    let set_pitch_shift_task = Task::done(Message::PitchShiftChanged(preset.pitch_shift_semitones));

    Task::batch(vec![
        set_stage_task,
        set_ir_task,
        set_ir_gain_task,
        set_pitch_shift_task,
    ])
}
