// TODO: Merge the contents of this file with `src/io/processor.rs`
use anyhow::{Context, Result};
use crossbeam::channel::{Sender, bounded};
use jack::{AsyncClient, Client, ClientOptions};
use log::error;

use crate::io::processor::Processor;
use crate::io::recorder::Recorder;
use crate::sim::chain::AmplifierChain;

/// Manages the audio processing chain and JACK client
pub struct ProcessorManager {
    _active_client: AsyncClient<Notifications, Processor>,
    recorder: Option<Recorder>,
    /// GUI → audio thread: push a completely new preset
    amp_tx: Sender<Box<AmplifierChain>>,
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

        let (tx_amp, rx_amp) = bounded::<Box<AmplifierChain>>(1);

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
            amp_tx: tx_amp,
        })
    }

    /// Push a new amplifier chain from the GUI side.
    /// Never blocks; silently drops if the buffer is full.
    pub fn set_amp_chain(&self, new_chain: AmplifierChain) {
        self.amp_tx
            .try_send(Box::new(new_chain))
            .unwrap_or_else(|e| {
                error!("Failed to send new amplifier chain: {e}");
            });
    }

    /// Starts recording if enabled
    pub fn enable_recording(&mut self) -> Result<()> {
        if self.recorder.is_some() {
            return Ok(());
        }

        Ok(())
    }

    /// Stops recording if active
    pub fn disable_recording(&mut self) {
        if self.recorder.is_none() {
            return;
        }

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
