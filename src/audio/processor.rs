use std::path::Path;

use crate::audio::ports::Ports;
use crate::audio::recorder::Recorder;
use crate::audio::samplers::Samplers;
use crate::ir::cabinet::IrCabinet;
use crate::sim::chain::AmplifierChain;
use crate::sim::tuner::Tuner;
use anyhow::{Context, Result};
use crossbeam::channel::Receiver;
use jack::{Client, Control, Frames, ProcessHandler, ProcessScope};
use log::{debug, error, warn};

pub enum ProcessorMessage {
    SetAmpChain(Box<AmplifierChain>),
    StartRecording(Recorder),
    StopRecording(),
    SetIrCabinet(Option<String>),
    SetIrBypass(bool),
    SetIrGain(f32),
    SetTunerEnabled(bool),
}

pub struct Processor {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// IR Cabinet processor
    ir_cabinet: Option<IrCabinet>,
    /// Channel for updating the amplifier chain.
    rx_updates: Receiver<ProcessorMessage>,
    audio_ports: Ports,
    samplers: Samplers,
    tuner: Tuner,
    recorder: Option<Recorder>,
}

impl Processor {
    pub fn new(
        client: &Client,
        rx_updates: Receiver<ProcessorMessage>,
        oversample_factor: f64,
        tuner: Tuner,
    ) -> Result<Self> {
        let audio_ports = Ports::new(client).context("failed to create audio ports manager")?;
        let samplers = Samplers::new(client.buffer_size() as usize, oversample_factor)
            .context("failed to create samplers")?;

        let ir_cabinet = match IrCabinet::new(
            Path::new("./impulse_responses"),
            client.sample_rate() as u32,
        ) {
            Ok(cab) => {
                debug!("IR Cabinet loaded successfully");
                Some(cab)
            }
            Err(e) => {
                warn!("Failed to load IR Cabinet: {}", e);
                None
            }
        };

        debug_stats(client);

        Ok(Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            rx_updates,
            audio_ports,
            samplers,
            tuner,
            recorder: None,
        })
    }

    pub fn handle_messages(&mut self) {
        if let Ok(message) = self.rx_updates.try_recv() {
            match message {
                ProcessorMessage::SetAmpChain(chain) => {
                    self.chain = chain;
                    debug!("Received new amplifier chain");
                }
                ProcessorMessage::SetIrCabinet(ir_name) => {
                    if let Some(ref mut cab) = self.ir_cabinet
                        && let Some(name) = ir_name
                    {
                        if let Err(e) = cab.select_ir(&name) {
                            error!("Failed to set IR: {}", e);
                        } else {
                            debug!("IR Cabinet set to: {}", name);
                        }
                    }
                }
                ProcessorMessage::SetIrBypass(bypass) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_bypass(bypass);
                        debug!("IR Cabinet bypass: {}", bypass);
                    }
                }
                ProcessorMessage::SetIrGain(gain) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_gain(gain);
                        debug!("IR Cabinet gain: {}", gain);
                    }
                }
                ProcessorMessage::SetTunerEnabled(enabled) => {
                    self.tuner.set_enabled(enabled);
                }
                ProcessorMessage::StartRecording(recorder) => {
                    if self.recorder.is_some() {
                        debug!("Recorder already active, ignoring start request");
                        return;
                    }

                    debug!("Recorder updated");
                    self.recorder = Some(recorder);
                }
                ProcessorMessage::StopRecording() => {
                    if self.recorder.is_none() {
                        debug!("No active recorder to stop");
                        return;
                    }

                    debug!("Stopping recorder");
                    if let Some(recorder) = self.recorder.take()
                        && let Err(e) = recorder.stop()
                    {
                        error!("Failed to stop recorder: {e}");
                    }

                    self.recorder = None;
                }
            }
        }
    }
}

impl ProcessHandler for Processor {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        self.handle_messages();

        let input = self.audio_ports.read_input(ps);
        self.samplers.copy_input(input);

        if self.tuner.is_enabled() {
            self.tuner.process(input);

            // No output
            self.audio_ports.silence_output(ps);
            return Control::Continue;
        }

        let upsampled = match self.samplers.upsample() {
            Ok(buf) => buf,
            Err(e) => {
                error!("Upsampling error: {}", e);
                return Control::Continue;
            }
        };

        let chain = self.chain.as_mut();
        for s in upsampled.iter_mut() {
            *s = chain.process(*s);
        }

        let downsampled = match self.samplers.downsample() {
            Ok(buf) => buf,
            Err(e) => {
                error!("Downsampling error: {}", e);
                return Control::Continue;
            }
        };

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(downsampled);
        }

        self.audio_ports.write_output(ps, downsampled);

        #[allow(clippy::collapsible_if)]
        if let Some(recorder) = self.recorder.as_mut() {
            if let Err(e) = recorder.record_block(downsampled) {
                error!("Error recording audio block: {e}");
            }
        }

        Control::Continue
    }

    fn buffer_size(&mut self, client: &Client, frames: Frames) -> Control {
        let new_size = frames as usize;
        warn!("buffer_size changed to {new_size} frames");
        debug_stats(client);

        if let Err(e) = self.samplers.resize_buffers(new_size) {
            error!("Failed to resize samplers: {}", e);
        }

        Control::Continue
    }
}

fn debug_stats(client: &Client) {
    let sample_rate = client.sample_rate() as f32;
    let buffer_frames = client.buffer_size() as f32;
    debug!(
        "Sample rate: {sample_rate}, Buffer frames: {buffer_frames}, Calls p/s: {}",
        sample_rate / buffer_frames
    );
}

impl Drop for Processor {
    fn drop(&mut self) {
        if let Some(recorder) = self.recorder.take() {
            debug!("Finalizing recorder on processor drop");
            if let Err(e) = recorder.stop() {
                error!("Failed to stop recorder: {e}");
            }
        }
    }
}
