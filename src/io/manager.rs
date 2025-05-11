use crossbeam::channel::{Sender, bounded};
use jack::{AsyncClient, Client, ClientOptions, contrib::ClosureProcessHandler};

use crate::io::processor::{ProcessHandler, Processor};
use crate::io::recorder::Recorder;
use crate::sim::chain::AmplifierChain;

/// Manages the audio processing chain and JACK client
pub struct ProcessorManager {
    client: Option<Client>,
    active_client: Option<AsyncClient<Notifications, ClosureProcessHandler<(), ProcessHandler>>>,
    recorder: Option<Recorder>,
    /// GUI → audio thread: push a completely new preset
    amp_tx: Option<Sender<Box<AmplifierChain>>>,
    sample_rate: f32,
}

/// JACK notifications handler
struct Notifications;
impl jack::NotificationHandler for Notifications {}

impl ProcessorManager {
    /// Creates a new ProcessorManager
    pub fn new() -> Result<Self, String> {
        let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .map_err(|e| format!("Failed to create JACK client: {e}"))?;

        Ok(Self {
            sample_rate: client.sample_rate() as f32,
            client: Some(client),
            active_client: None,
            recorder: None,
            amp_tx: None,
        })
    }

    /// Push a new amplifier chain from the GUI side.
    /// Never blocks; silently drops if the buffer is full.
    pub fn set_amp_chain(&self, new_chain: AmplifierChain) {
        if let Some(tx) = &self.amp_tx {
            let _ = tx.try_send(Box::new(new_chain));
        }
    }

    /// Starts recording if enabled
    pub fn enable_recording(&mut self, record_dir: &str) -> Result<(), String> {
        if self.recorder.is_some() {
            return Ok(()); // Already recording
        }

        let recorder = Recorder::new(self.sample_rate as u32, record_dir)
            .map_err(|e| format!("Failed to start recorder: {e}"))?;

        self.recorder = Some(recorder);
        Ok(())
    }

    /// Stops recording if active
    pub fn disable_recording(&mut self) {
        if let Some(recorder) = self.recorder.take() {
            recorder.stop();
        }
    }

    /// Starts the audio processing
    pub fn start(&mut self) -> Result<(), String> {
        if self.active_client.is_some() {
            return Ok(());
        }

        let client = self.client.take().ok_or("Client already active")?;
        let (tx_amp, rx_amp) = bounded::<Box<AmplifierChain>>(1); // SPSC, size 1

        let tx_audio = self.recorder.as_ref().map(|r| r.sender());

        // Processor owns its mutable chain and a receiver for updates
        let processor = Processor::new_with_channel(&client, rx_amp, tx_audio);

        let callback = processor.into_process_handler();
        let handler = ClosureProcessHandler::new(callback);
        let active = client
            .activate_async(Notifications, handler)
            .map_err(|e| format!("activate_async: {e}"))?;

        self.amp_tx = Some(tx_amp);
        self.active_client = Some(active);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(active) = self.active_client.take() {
            let (client, _n, _h) = active
                .deactivate()
                .map_err(|e| format!("deactivate: {e:?}"))?;
            self.client = Some(client);
        }
        self.disable_recording();
        self.amp_tx = None;
        Ok(())
    }

    /// Returns the sample rate
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

impl Drop for ProcessorManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
