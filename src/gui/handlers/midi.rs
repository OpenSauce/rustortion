use iced::{Element, Task};
use log::debug;

use crate::gui::components::dialogs::midi::MidiDialog;
use crate::gui::messages::{Message, MidiMessage, PresetMessage};
use crate::midi::{MidiEvent, MidiHandle, MidiMapping};

pub struct MidiHandler {
    dialog: MidiDialog,
    handle: MidiHandle,
}

impl MidiHandler {
    pub fn new(handle: MidiHandle) -> Self {
        Self {
            dialog: MidiDialog::new(),
            handle,
        }
    }

    pub fn handle(
        &mut self,
        message: MidiMessage,
        presets: Vec<String>,
        mappings: &[MidiMapping],
    ) -> Task<Message> {
        match message {
            MidiMessage::Open => {
                self.dialog.show(presets, mappings.to_vec());
            }
            MidiMessage::Close => {
                self.dialog.hide();
            }
            MidiMessage::ControllerSelected(controller_name) => {
                self.dialog
                    .set_selected_controller(Some(controller_name.clone()));
                self.handle.connect(&controller_name);
                return Task::none();
            }
            MidiMessage::Disconnect => {
                self.handle.disconnect();
                self.dialog.set_selected_controller(None);
            }
            MidiMessage::RefreshControllers => {
                self.dialog.refresh_controllers();
            }
            MidiMessage::StartLearning => {
                self.dialog.start_learning();
            }
            MidiMessage::CancelLearning => {
                self.dialog.cancel_learning();
            }
            MidiMessage::PresetForMappingSelected(preset) => {
                self.dialog.set_preset_for_mapping(preset);
            }
            MidiMessage::ConfirmMapping => {
                if self.dialog.complete_mapping().is_some() {
                    let mappings = self.dialog.get_mappings();
                    self.handle.set_mappings(mappings);
                    debug!("MIDI mapping added and saved");
                    return Task::none();
                }
            }
            MidiMessage::RemoveMapping(idx) => {
                self.dialog.remove_mapping(idx);
                let mappings = self.dialog.get_mappings();
                self.handle.set_mappings(mappings);
                debug!("MIDI mapping removed and saved");
                return Task::none();
            }
            MidiMessage::Update => {
                return self.poll_events();
            }
        }

        Task::none()
    }

    fn poll_events(&mut self) -> Task<Message> {
        while let Some(event) = self.handle.try_recv() {
            match event {
                MidiEvent::Input(input) => {
                    if self.dialog.is_visible() {
                        self.dialog.on_midi_input(&input);
                    }

                    if self.dialog.is_learning() {
                        continue;
                    }

                    if let Some(preset_name) = self.handle.check_mapping(&input) {
                        debug!("MIDI triggered preset: {}", preset_name);
                        return Task::done(Message::Preset(PresetMessage::Select(preset_name)));
                    }
                }
                MidiEvent::Disconnected => {
                    self.dialog.set_selected_controller(None);
                    debug!("MIDI device disconnected");
                }
                MidiEvent::Error(e) => {
                    log::error!("MIDI error: {}", e);
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Option<Element<'_, Message>> {
        self.dialog.view().map(|e| e.map(Message::Midi))
    }

    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    pub fn get_selected_controller(&self) -> Option<String> {
        self.dialog.get_selected_controller()
    }

    pub fn set_mappings(&mut self, mappings: Vec<MidiMapping>) {
        self.dialog.set_mappings(mappings.clone());
        self.handle.set_mappings(mappings);
    }

    pub fn get_mappings(&self) -> Vec<MidiMapping> {
        self.dialog.get_mappings()
    }

    pub fn connect(&mut self, device_name: &str) {
        self.handle.connect(device_name);
        self.dialog
            .set_selected_controller(Some(device_name.to_owned()));
    }

    pub fn set_selected_controller(&mut self, controller: Option<String>) {
        self.dialog.set_selected_controller(controller);
    }
}
