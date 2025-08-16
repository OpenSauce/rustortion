use anyhow::{Context, Result};
use crossbeam::channel::{Sender, bounded};
use jack::{AsyncClient, Client, ClientOptions};
use log::{error, info, warn};

use crate::gui::settings::AudioSettings;
use crate::io::processor::{Processor, ProcessorMessage};
use crate::io::recorder::Recorder;
use crate::sim::chain::AmplifierChain;

/// Manages the audio processing chain and JACK client
pub struct ProcessorManager {
    active_client: AsyncClient<Notifications, Processor>,
    recorder: Option<Recorder>,
    /// GUI → audio thread: push a completely new preset
    tx_updates: Sender<ProcessorMessage>,
    sample_rate: f32,
    current_settings: AudioSettings,
}

/// JACK notifications handler
struct Notifications;
impl jack::NotificationHandler for Notifications {}

impl ProcessorManager {
    /// Creates a new ProcessorManager
    pub fn new(settings: AudioSettings, auto_connect: bool) -> Result<Self> {
        let (client, _) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .context("failed to create JACK client")?;

        let (tx_amp, rx_amp) = bounded::<ProcessorMessage>(10);

        let processor =
            Processor::new(&client, rx_amp, None, &settings).context("error creating processor")?;

        let sample_rate = client.sample_rate() as f32;

        let active_client = client
            .activate_async(Notifications, processor)
            .context("failed to activate async client")?;

        let mut manager = Self {
            active_client,
            recorder: None,
            tx_updates: tx_amp,
            sample_rate,
            current_settings: settings.clone(),
        };

        // Auto-connect if requested
        if auto_connect && settings.auto_connect {
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
                error!("Failed to disconnect in_port: {e}");
            });
        }

        if let Some(port) = client.port_by_name("rustortion:out_port_right") {
            client.disconnect(&port).unwrap_or_else(|e| {
                error!("Failed to disconnect in_port: {e}");
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
        let update = ProcessorMessage::SetAmpChain(Box::new(new_chain));
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send new amplifier chain: {e}");
        });
    }

    /// Starts recording if enabled
    pub fn enable_recording(&mut self) -> Result<()> {
        if self.recorder.is_some() {
            return Ok(());
        }

        let recorder = Recorder::new(self.sample_rate as u32, "./recordings")?;
        let audio_tx = recorder.sender();

        let update = ProcessorMessage::SetRecording(Some(audio_tx));
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send recording update: {e}");
        });

        self.recorder = Some(recorder);
        Ok(())
    }

    /// Stops recording if active
    pub fn disable_recording(&mut self) {
        if self.recorder.is_none() {
            return;
        }

        let update = ProcessorMessage::SetRecording(None);
        self.tx_updates.try_send(update).unwrap_or_else(|e| {
            error!("Failed to send recording update: {e}");
        });

        if let Some(recorder) = self.recorder.take()
            && let Err(e) = recorder.stop()
        {
            error!("Failed to stop recorder: {e}");
        }
    }

    /// Returns the sample rate
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

impl std::fmt::Debug for ProcessorManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessorManager")
            .field("sample_rate", &self.sample_rate)
            .field("recorder", &self.recorder.is_some())
            .finish()
    }
}

impl Drop for ProcessorManager {
    fn drop(&mut self) {
        self.disable_recording();
    }
}
