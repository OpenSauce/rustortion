use arc_swap::ArcSwap;
use crossbeam::channel::{Receiver, Sender, bounded};
use log::{debug, error, info, warn};
use midir::{MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;

/// A MIDI input mapping that associates a MIDI message with a preset
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MidiMapping {
    /// The MIDI channel (0-15)
    pub channel: u8,
    /// The MIDI control/note number
    pub control: u8,
    /// The preset name to load when this input is triggered
    pub preset_name: String,
    /// Human-readable description of this mapping
    pub description: String,
}

impl MidiMapping {
    pub fn new(channel: u8, control: u8, preset_name: String) -> Self {
        Self {
            channel,
            control,
            preset_name,
            description: format!("Ch{} CC/Note {}", channel + 1, control),
        }
    }

    /// Check if this mapping matches the given MIDI message
    pub fn matches(&self, channel: u8, control: u8) -> bool {
        self.channel == channel && self.control == control
    }
}

/// Represents a detected MIDI input
#[derive(Debug, Clone)]
pub struct MidiInputEvent {
    pub channel: u8,
    pub message_type: MidiMessageType,
    pub control: u8,
    pub value: u8,
    pub raw_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiMessageType {
    NoteOn,
    NoteOff,
    ControlChange,
    ProgramChange,
    Other,
}

impl std::fmt::Display for MidiMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiMessageType::NoteOn => write!(f, "Note On"),
            MidiMessageType::NoteOff => write!(f, "Note Off"),
            MidiMessageType::ControlChange => write!(f, "CC"),
            MidiMessageType::ProgramChange => write!(f, "Program"),
            MidiMessageType::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for MidiInputEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ch{} {} #{} val={}",
            self.channel + 1,
            self.message_type,
            self.control,
            self.value
        )
    }
}

/// Messages sent from the MIDI thread to the main application
#[derive(Debug, Clone)]
pub enum MidiEvent {
    /// A MIDI input was received
    Input(MidiInputEvent),
    /// Connection was lost
    Disconnected,
    /// Error occurred
    Error(String),
}

/// Commands sent from the main application to the MIDI manager
pub enum MidiCommand {
    /// Connect to a specific device
    Connect(String),
    /// Disconnect from current device
    Disconnect,
    /// Shutdown the MIDI thread
    Shutdown,
}

/// Handle for communicating with the MIDI manager from the main thread
pub struct MidiHandle {
    command_sender: Sender<MidiCommand>,
    event_receiver: Receiver<MidiEvent>,
    mappings: Arc<ArcSwap<Vec<MidiMapping>>>,
}

impl MidiHandle {
    pub fn connect(&self, device_name: &str) {
        if let Err(e) = self
            .command_sender
            .try_send(MidiCommand::Connect(device_name.to_string()))
        {
            error!("Failed to send connect command: {}", e);
        }
    }

    pub fn disconnect(&self) {
        if let Err(e) = self.command_sender.try_send(MidiCommand::Disconnect) {
            error!("Failed to send disconnect command: {}", e);
        }
    }

    pub fn try_recv(&self) -> Option<MidiEvent> {
        self.event_receiver.try_recv().ok()
    }

    pub fn set_mappings(&self, mappings: Vec<MidiMapping>) {
        self.mappings.store(Arc::new(mappings));
    }

    pub fn get_mappings(&self) -> Vec<MidiMapping> {
        self.mappings.load().as_ref().clone()
    }

    /// Check if a MIDI input matches any mapping and return the preset name
    pub fn check_mapping(&self, event: &MidiInputEvent) -> Option<String> {
        let mappings = self.mappings.load();
        for mapping in mappings.iter() {
            if mapping.matches(event.channel, event.control) {
                return Some(mapping.preset_name.clone());
            }
        }
        None
    }
}

impl Drop for MidiHandle {
    fn drop(&mut self) {
        let _ = self.command_sender.try_send(MidiCommand::Shutdown);
    }
}

/// The MIDI manager runs in a separate thread and handles device connections
pub struct MidiManager {
    command_receiver: Receiver<MidiCommand>,
    event_sender: Sender<MidiEvent>,
    connection: Option<MidiInputConnection<()>>,
    midi_event_sender: Sender<MidiEvent>,
}

impl MidiManager {
    /// Create a new MIDI manager and its handle
    pub fn new() -> (Self, MidiHandle) {
        let (command_sender, command_receiver) = bounded(10);
        let (event_sender, event_receiver) = bounded(100);
        let mappings = Arc::new(ArcSwap::from_pointee(Vec::new()));

        (
            Self {
                command_receiver,
                event_sender: event_sender.clone(),
                connection: None,
                midi_event_sender: event_sender,
            },
            MidiHandle {
                command_sender,
                event_receiver,
                mappings,
            },
        )
    }

    /// Get a list of available MIDI input devices
    pub fn list_devices() -> Vec<String> {
        match MidiInput::new("rustortion-scan") {
            Ok(midi_in) => midi_in
                .ports()
                .iter()
                .filter_map(|p| midi_in.port_name(p).ok())
                .collect(),
            Err(e) => {
                error!("Failed to create MIDI input for scanning: {}", e);
                Vec::new()
            }
        }
    }

    /// Run the MIDI manager (blocking, should be called from a dedicated thread)
    pub fn run(mut self) {
        debug!("MIDI manager started");

        loop {
            match self.command_receiver.recv() {
                Ok(MidiCommand::Connect(device_name)) => {
                    self.handle_connect(&device_name);
                }
                Ok(MidiCommand::Disconnect) => {
                    self.handle_disconnect();
                }
                Ok(MidiCommand::Shutdown) => {
                    debug!("MIDI manager shutting down");
                    self.handle_disconnect();
                    break;
                }
                Err(_) => {
                    // Channel closed, shutdown
                    break;
                }
            }
        }
    }

    fn handle_connect(&mut self, device_name: &str) {
        // Disconnect existing connection first
        self.handle_disconnect();

        let midi_in = match MidiInput::new("rustortion") {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create MIDI input: {}", e);
                let _ = self.event_sender.try_send(MidiEvent::Error(format!(
                    "Failed to create MIDI input: {}",
                    e
                )));
                return;
            }
        };

        // Find the port by name
        let port = midi_in.ports().into_iter().find(|p| {
            midi_in
                .port_name(p)
                .map(|n| n == device_name)
                .unwrap_or(false)
        });

        let port = match port {
            Some(p) => p,
            None => {
                error!("MIDI device not found: {}", device_name);
                let _ = self.event_sender.try_send(MidiEvent::Error(format!(
                    "Device not found: {}",
                    device_name
                )));
                return;
            }
        };

        let sender = self.midi_event_sender.clone();

        let connection = match midi_in.connect(
            &port,
            "rustortion-input",
            move |_timestamp, message, _| {
                let Some(event) = parse_midi_message(message) else {
                    return;
                };

                if let Err(e) = sender.try_send(MidiEvent::Input(event)) {
                    warn!("Failed to send MIDI event: {}", e);
                }
            },
            (),
        ) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to MIDI device: {}", e);
                return;
            }
        };

        info!("Connected to MIDI device: {}", device_name);
        self.connection = Some(connection);
    }

    fn handle_disconnect(&mut self) {
        if let Some(conn) = self.connection.take() {
            conn.close();
            info!("Disconnected from MIDI device");
        }
    }
}

/// Parse raw MIDI bytes into a MidiInputEvent
fn parse_midi_message(message: &[u8]) -> Option<MidiInputEvent> {
    if message.is_empty() {
        return None;
    }

    let status = message[0];
    let message_type = status & 0xF0;
    let channel = status & 0x0F;

    let (msg_type, control, value) = match message_type {
        0x90 => {
            // Note On
            if message.len() >= 3 {
                let note = message[1];
                let velocity = message[2];
                if velocity == 0 {
                    (MidiMessageType::NoteOff, note, velocity)
                } else {
                    (MidiMessageType::NoteOn, note, velocity)
                }
            } else {
                return None;
            }
        }
        0x80 => {
            // Note Off
            if message.len() >= 3 {
                (MidiMessageType::NoteOff, message[1], message[2])
            } else {
                return None;
            }
        }
        0xB0 => {
            // Control Change
            if message.len() >= 3 {
                (MidiMessageType::ControlChange, message[1], message[2])
            } else {
                return None;
            }
        }
        0xC0 => {
            // Program Change
            if message.len() >= 2 {
                (MidiMessageType::ProgramChange, message[1], 0)
            } else {
                return None;
            }
        }
        _ => (
            MidiMessageType::Other,
            message.get(1).copied().unwrap_or(0),
            message.get(2).copied().unwrap_or(0),
        ),
    };

    Some(MidiInputEvent {
        channel,
        message_type: msg_type,
        control,
        value,
        raw_bytes: message.to_vec(),
    })
}

/// Start the MIDI manager in a background thread
pub fn start_midi_manager() -> MidiHandle {
    let (manager, handle) = MidiManager::new();

    thread::Builder::new()
        .name("midi-manager".to_string())
        .spawn(move || {
            manager.run();
        })
        .expect("Failed to spawn MIDI manager thread");

    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_on() {
        let message = [0x90, 60, 100]; // Note On, channel 0, note 60, velocity 100
        let event = parse_midi_message(&message).unwrap();
        assert_eq!(event.channel, 0);
        assert_eq!(event.message_type, MidiMessageType::NoteOn);
        assert_eq!(event.control, 60);
        assert_eq!(event.value, 100);
    }

    #[test]
    fn test_parse_note_off_via_velocity() {
        let message = [0x90, 60, 0]; // Note On with velocity 0 = Note Off
        let event = parse_midi_message(&message).unwrap();
        assert_eq!(event.message_type, MidiMessageType::NoteOff);
    }

    #[test]
    fn test_parse_control_change() {
        let message = [0xB1, 7, 64]; // CC, channel 1, control 7, value 64
        let event = parse_midi_message(&message).unwrap();
        assert_eq!(event.channel, 1);
        assert_eq!(event.message_type, MidiMessageType::ControlChange);
        assert_eq!(event.control, 7);
        assert_eq!(event.value, 64);
    }

    #[test]
    fn test_midi_mapping_matches() {
        let mapping = MidiMapping::new(0, 60, "Test Preset".to_string());
        assert!(mapping.matches(0, 60));
        assert!(!mapping.matches(1, 60));
        assert!(!mapping.matches(0, 61));
    }
}
