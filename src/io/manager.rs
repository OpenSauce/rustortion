use anyhow::{Context, Result};
use crossbeam::channel::{Sender, bounded};
use jack::{AsyncClient, Client, ClientOptions};
use log::error;

use crate::io::processor::{Processor, ProcessorMessage};
use crate::io::recorder::Recorder;
use crate::sim::chain::AmplifierChain;

/// Manages the audio processing chain and JACK client
pub struct ProcessorManager {
    _active_client: AsyncClient<Notifications, Processor>,
    recorder: Option<Recorder>,
    /// GUI → audio thread: push a completely new preset
    tx_updates: Sender<ProcessorMessage>,
    sample_rate: f32,
}

/// JACK notifications handler
struct Notifications;
impl jack::NotificationHandler for Notifications {}

impl ProcessorManager {
    /// Creates a new ProcessorManager
    pub fn new(recorder: Option<Recorder>) -> Result<Self> {
        let (client, _) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .context("failed to create JACK client")?;

        let (tx_amp, rx_amp) = bounded::<ProcessorMessage>(10);

        let processor =
            Processor::new(&client, rx_amp, None).context("error creating processor")?;

        let sample_rate = client.sample_rate() as f32;

        let _active_client = client
            .activate_async(Notifications, processor)
            .context("failed to activate async client")?;

        Ok(Self {
            sample_rate,
            _active_client,
            recorder,
            tx_updates: tx_amp,
        })
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
            error!("Failed to send new amplifier chain: {e}");
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

        if let Some(recorder) = self.recorder.take() {
            if let Err(e) = recorder.stop() {
                error!("Failed to stop recorder: {e}");
            }
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
