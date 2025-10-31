use crossbeam::channel::{Receiver, Sender, bounded};
use nih_plug::prelude::*;
use std::sync::Arc;
use std::thread;

use super::params::RustortionParams;
use crate::gui::config::StageConfig;

#[derive(Debug, Clone)]
pub enum PluginToGui {
    UpdateStages(Vec<StageConfig>),
    UpdateGain(f32),
}

#[derive(Debug, Clone)]
pub enum GuiToPlugin {
    StagesChanged(Vec<StageConfig>),
}

pub struct ThreadedEditor {
    params: Arc<RustortionParams>,
    gui_thread: Option<thread::JoinHandle<()>>,
    tx_to_gui: Sender<PluginToGui>,
    rx_from_gui: Receiver<GuiToPlugin>,
}

impl ThreadedEditor {
    pub fn new(params: Arc<RustortionParams>) -> Self {
        let (tx_to_gui, rx_in_gui) = bounded::<PluginToGui>(100);
        let (tx_from_gui, rx_from_gui) = bounded::<GuiToPlugin>(100);

        // Clone params for the GUI thread
        let params_clone = params.clone();

        // Spawn the GUI thread
        let gui_thread = thread::spawn(move || {
            run_gui_thread(params_clone, rx_in_gui, tx_from_gui);
        });

        Self {
            params,
            gui_thread: Some(gui_thread),
            tx_to_gui,
            rx_from_gui,
        }
    }

    pub fn update(&mut self) {
        // Check for messages from GUI thread
        while let Ok(msg) = self.rx_from_gui.try_recv() {
            match msg {
                GuiToPlugin::StagesChanged(stages) => {
                    log::info!("Received stages from GUI: {} stages", stages.len());
                    if let Ok(json) = serde_json::to_string(&stages) {
                        *self.params.stages_json.lock() = Some(json);
                        self.params
                            .stages_changed
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        }
    }

    pub fn send_update(&self, msg: PluginToGui) {
        let _ = self.tx_to_gui.try_send(msg);
    }
}

impl Drop for ThreadedEditor {
    fn drop(&mut self) {
        // GUI thread will exit when channels are dropped
        if let Some(handle) = self.gui_thread.take() {
            let _ = handle.join();
        }
    }
}

fn run_gui_thread(
    params: Arc<RustortionParams>,
    rx_from_plugin: Receiver<PluginToGui>,
    tx_to_plugin: Sender<GuiToPlugin>,
) {
    use crate::gui::app::AmplifierApp;
    use crate::gui::settings::Settings;
    use iced::{Font, Theme, window};

    log::info!("Starting GUI thread");

    // Load stages from params
    let initial_stages = if let Some(json) = params.stages_json.lock().as_ref() {
        serde_json::from_str(json).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Create a custom app that uses channels instead of ProcessorManager
    let app = GuiOnlyApp::new(initial_stages, tx_to_plugin, rx_from_plugin);

    // Start Iced application in this thread
    let result = iced::application("Rustortion", GuiOnlyApp::update, GuiOnlyApp::view)
        .window(window::Settings {
            size: iced::Size::new(800.0, 600.0),
            decorations: true,
            ..window::Settings::default()
        })
        .theme(|_| Theme::TokyoNight)
        .default_font(Font::MONOSPACE);

    log::info!("GUI thread exiting");
}

// Simplified app that doesn't use ProcessorManager
struct GuiOnlyApp {
    stages: Vec<StageConfig>,
    tx_to_plugin: Sender<GuiToPlugin>,
    rx_from_plugin: Receiver<PluginToGui>,
    // Add your GUI components here
    stage_list: crate::gui::components::stage_list::StageList,
    control_bar: crate::gui::components::control::Control,
}

impl GuiOnlyApp {
    fn new(
        stages: Vec<StageConfig>,
        tx_to_plugin: Sender<GuiToPlugin>,
        rx_from_plugin: Receiver<PluginToGui>,
    ) -> Self {
        use crate::gui::config::StageType;

        let stage_list = crate::gui::components::stage_list::StageList::new(stages.clone());
        let control_bar = crate::gui::components::control::Control::new(StageType::default());

        Self {
            stages,
            tx_to_plugin,
            rx_from_plugin,
            stage_list,
            control_bar,
        }
    }

    fn update(
        &mut self,
        message: crate::gui::messages::Message,
    ) -> iced::Task<crate::gui::messages::Message> {
        use crate::gui::config::StageConfig;
        use crate::gui::messages::Message;

        // Check for updates from plugin
        while let Ok(msg) = self.rx_from_plugin.try_recv() {
            match msg {
                PluginToGui::UpdateStages(stages) => {
                    self.stages = stages;
                    self.stage_list.set_stages(&self.stages);
                }
                PluginToGui::UpdateGain(_gain) => {
                    // Update gain display if you have one
                }
            }
        }

        // Handle GUI messages
        match message {
            Message::AddStage => {
                let new_stage = StageConfig::from(self.control_bar.selected());
                self.stages.push(new_stage);
                self.send_stages_to_plugin();
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                    self.send_stages_to_plugin();
                }
            }
            Message::Stage(idx, stage_msg) => {
                if let Some(stage) = self.stages.get_mut(idx) {
                    if stage.apply(stage_msg) {
                        self.send_stages_to_plugin();
                    }
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.control_bar.set_selected_stage_type(stage_type);
            }
            Message::MoveStageUp(idx) => {
                if idx > 0 && idx < self.stages.len() {
                    self.stages.swap(idx - 1, idx);
                    self.send_stages_to_plugin();
                }
            }
            Message::MoveStageDown(idx) => {
                if idx + 1 < self.stages.len() {
                    self.stages.swap(idx, idx + 1);
                    self.send_stages_to_plugin();
                }
            }
            _ => {} // Ignore recording, presets, settings for now
        }

        self.stage_list.set_stages(&self.stages);
        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, crate::gui::messages::Message> {
        use iced::widget::{Space, column, container};
        use iced::{Alignment, Length};

        let content = column![
            Space::new(Length::Fill, Length::Fixed(20.0)),
            self.stage_list.view(),
            self.control_bar.view(false), // No recording in plugin
        ]
        .spacing(20)
        .padding(20)
        .align_x(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn send_stages_to_plugin(&self) {
        let _ = self
            .tx_to_plugin
            .try_send(GuiToPlugin::StagesChanged(self.stages.clone()));
    }
}

// Wrapper for NIH-plug Editor trait
pub struct ThreadedEditorWrapper {
    editor: Option<ThreadedEditor>,
}

impl ThreadedEditorWrapper {
    pub fn new(params: Arc<RustortionParams>) -> Self {
        Self {
            editor: Some(ThreadedEditor::new(params)),
        }
    }
}

impl Editor for ThreadedEditorWrapper {
    fn spawn(
        &self,
        _parent: ParentWindowHandle,
        _context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        log::info!("GUI thread already spawned in constructor");
        Box::new(())
    }

    fn size(&self) -> (u32, u32) {
        (800, 600)
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        // Forward to GUI thread if needed
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}

    fn param_values_changed(&self) {
        if let Some(ref editor) = self.editor {
            // Could send updates here
        }
    }
}
