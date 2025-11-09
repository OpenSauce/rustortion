use anyhow::{Context, Result};
use crossbeam::channel::{Sender, bounded};
use jack::{AsyncClient, Client, ClientOptions};
use log::{error, info, warn};

use crate::audio::engine::{Engine, EngineMessage};
use crate::audio::jack::{NotificationHandler, ProcessHandler};
use crate::audio::recorder::Recorder;
use crate::gui::settings::AudioSettings;
use crate::sim::chain::AmplifierChain;
use crate::sim::tuner::{Tuner, TunerHandle, TunerInfo};

/// Manages the audio processing chain and JACK client
pub struct Manager {
    active_client: AsyncClient<NotificationHandler, ProcessHandler>,
    /// GUI → audio thread: push a completely new preset
    tx_updates: Sender<EngineMessage>,
    sample_rate: usize,
    current_settings: AudioSettings,
    tuner_handle: TunerHandle,
}

impl Manager {
    pub fn new(settings: AudioSettings) -> Result<Self> {
        let (client, _) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .context("failed to create JACK client")?;

        let sample_rate = client.sample_rate();
        let buffer_size = client.buffer_size() as usize;

        let (tx_amp, rx_amp) = bounded::<EngineMessage>(10);
        let (tuner, handle) = Tuner::new(sample_rate);

        let engine = Engine::new(
            rx_amp,
            settings.oversampling_factor.into(),
            tuner,
            buffer_size,
            sample_rate,
        )?;

        let jack_handler =
            ProcessHandler::new(&client, engine).context("failed to create process handler")?;

        let active_client = client
            .activate_async(NotificationHandler, jack_handler)
            .context("failed to activate async client")?;

        let mut manager = Self {
            active_client,
            tx_updates: tx_amp,
            sample_rate,
            current_settings: settings.clone(),
            tuner_handle: handle,
        };

        // Auto-connect if requested
        if settings.auto_connect {
            manager.connect_ports(&settings);
        }

        Ok(manager)
    }

    /// Connect audio ports based on settings
    fn connect_ports(&mut self, settings: &AudioSettings) {
        let client = &self.active_client.as_client();

        // Connect input
        if let Err(e) = client.connect_ports_by_name(&settings.input_port, "rustortion:in_port") {
            warn!(
                "Failed to connect input port '{}': {}",
                settings.input_port, e
            );
        } else {
            info!(
                "Connected input: {} -> rustortion:in_port",
                settings.input_port
            );
        }

        // Connect left output
        if let Err(e) =
            client.connect_ports_by_name("rustortion:out_port_left", &settings.output_left_port)
        {
            warn!(
                "Failed to connect left output port '{}': {}",
                settings.output_left_port, e
            );
        } else {
            info!(
                "Connected left output: rustortion:out_port_left -> {}",
                settings.output_left_port
            );
        }

        // Connect right output
        if let Err(e) =
            client.connect_ports_by_name("rustortion:out_port_right", &settings.output_right_port)
        {
            warn!(
                "Failed to connect right output port '{}': {}",
                settings.output_right_port, e
            );
        } else {
            info!(
                "Connected right output: rustortion:out_port_right -> {}",
                settings.output_right_port
            );
        }
    }

    /// Reconnect with new settings
    pub fn apply_settings(&mut self, new_settings: AudioSettings) -> Result<()> {
        info!("Applying new audio settings");

        // Disconnect existing connections
        self.disconnect_all();

        // Update settings
        self.current_settings = new_settings.clone();

        // Reconnect with new settings
        if new_settings.auto_connect {
            self.connect_ports(&new_settings);
        }

        Ok(())
    }

    /// Disconnect all audio connections
    pub fn disconnect_all(&self) {
        let client = self.active_client.as_client();

        // Get our ports
        if let Some(port) = client.port_by_name("rustortion:in_port") {
            client.disconnect(&port).unwrap_or_else(|e| {
                error!("Failed to disconnect in_port: {e}");
            });
        }

        if let Some(port) = client.port_by_name("rustortion:out_port_left") {
            client.disconnect(&port).unwrap_or_else(|e| {
                error!("Failed to disconnect out_port_left: {e}");
            });
        }

        if let Some(port) = client.port_by_name("rustortion:out_port_right") {
            client.disconnect(&port).unwrap_or_else(|e| {
                error!("Failed to disconnect out_port_right: {e}");
            });
        }
    }

    /// Get available input ports
    pub fn get_available_inputs(&self) -> Vec<String> {
        self.active_client
            .as_client()
            .ports(None, Some("audio"), jack::PortFlags::IS_OUTPUT)
            .into_iter()
            .filter(|p| !p.starts_with("rustortion:"))
            .collect()
    }

    /// Get available output ports
    pub fn get_available_outputs(&self) -> Vec<String> {
        self.active_client
            .as_client()
            .ports(None, Some("audio"), jack::PortFlags::IS_INPUT)
            .into_iter()
            .filter(|p| !p.starts_with("rustortion:"))
            .collect()
    }

    /// Push a new amplifier chain from the GUI side.
    /// Never blocks; silently drops if the buffer is full.
    pub fn set_amp_chain(&self, new_chain: AmplifierChain) {
        let update = EngineMessage::SetAmpChain(Box::new(new_chain));
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send new amplifier chain: {e}");
        });
    }

    /// Enables recording.
    pub fn enable_recording(&mut self) -> Result<()> {
        let recorder = Recorder::new(self.sample_rate as u32, "./recordings")?;

        let update = EngineMessage::StartRecording(recorder);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send recording update: {e}");
        });

        Ok(())
    }

    /// Disables recording.
    pub fn disable_recording(&mut self) {
        let update = EngineMessage::StopRecording();
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send recording update: {e}");
        });
    }

    /// Set the active IR cabinet
    pub fn set_ir_cabinet(&self, ir_name: Option<String>) {
        let update = EngineMessage::SetIrCabinet(ir_name);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send IR cabinet update: {e}");
        });
    }

    /// Set IR cabinet bypass state
    pub fn set_ir_bypass(&self, bypass: bool) {
        let update = EngineMessage::SetIrBypass(bypass);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send IR bypass update: {e}");
        });
    }

    /// Set IR cabinet gain level
    pub fn set_ir_gain(&self, gain: f32) {
        let update = EngineMessage::SetIrGain(gain);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send IR gain update: {e}");
        });
    }

    pub fn set_tuner_enabled(&self, enabled: bool) {
        let update = EngineMessage::SetTunerEnabled(enabled);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send tuner enable update: {e}");
        });
    }

    pub fn poll_tuner_info(&self) -> TunerInfo {
        self.tuner_handle.get_tuner_info()
    }

    /// Returns the sample rate
    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }
}
