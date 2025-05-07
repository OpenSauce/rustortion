use crossbeam::queue::ArrayQueue;
use jack::{Client, ClientOptions, contrib::ClosureProcessHandler};
use std::sync::{Arc, Mutex};

use crate::io::processor::Processor;
use crate::io::recorder::Recorder;
use crate::sim::chain::AmplifierChain;

/// Manages the audio processing chain and JACK client
pub struct ProcessorManager {
    client: Option<Client>,
    active_client: Option<
        jack::AsyncClient<Notifications, ClosureProcessHandler<&'static Client, jack::Control>>,
    >,
    recorder: Option<Recorder>,
    /* GUI edits this value behind a mutex … */
    gui_chain: Arc<Mutex<AmplifierChain>>,
    /* … and we push boxed clones into the queue the audio thread owns */
    inbox: Option<Arc<ArrayQueue<Box<AmplifierChain>>>>,
    sample_rate: f32,
}

/// JACK notifications handler
struct Notifications;
impl jack::NotificationHandler for Notifications {}

impl ProcessorManager {
    /// Creates a new ProcessorManager
    pub fn new() -> Result<Self, String> {
        // Set up JACK client
        let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .map_err(|e| format!("Failed to create JACK client: {}", e))?;

        Ok(Self {
            sample_rate: client.sample_rate() as f32,
            client: Some(client),
            active_client: None,
            recorder: None,
            gui_chain: Arc::new(Mutex::new(AmplifierChain::new("Default"))),
            inbox: None,
        })
    }

    /// Sets up a new amplifier chain
    pub fn set_amp_chain(&mut self, new_chain: AmplifierChain) {
        *self.gui_chain.lock().unwrap() = new_chain.clone(); // keep GUI copy
        if let Some(q) = &self.inbox {
            let _ = q.push(Box::new(new_chain)); // drop if full
        }
    }

    /// Gets a reference to the amplifier chain for GUI updates
    pub fn get_amp_chain(&self) -> Arc<Mutex<AmplifierChain>> {
        self.gui_chain.clone()
    }

    /// Starts recording if enabled
    pub fn enable_recording(&mut self, record_dir: &str) -> Result<(), String> {
        if self.recorder.is_some() {
            return Ok(()); // Already recording
        }

        let recorder = Recorder::new(self.sample_rate as u32, record_dir)
            .map_err(|e| format!("Failed to start recorder: {}", e))?;

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

        let tx = self.recorder.as_ref().map(|r| r.sender());

        /* build processor + get its inbox -------------------------------- */
        let init_chain = self.gui_chain.lock().unwrap().clone();
        let (processor, inbox) = Processor::new_shared(&client, init_chain, tx);

        self.inbox = Some(inbox); // store for future GUI pushes

        let callback = processor.into_process_handler();
        let active = client
            .activate_async(
                Notifications,
                ClosureProcessHandler::new(&client, callback), // D = &Client
            )
            .map_err(|e| format!("activate_async: {e}"))?;

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
