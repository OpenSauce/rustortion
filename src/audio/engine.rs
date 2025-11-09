use anyhow::{Context, Result};
use crossbeam::channel::Receiver;
use log::{debug, error, warn};
use std::path::Path;

use crate::audio::recorder::Recorder;
use crate::audio::samplers::Samplers;
use crate::ir::cabinet::IrCabinet;
use crate::sim::chain::AmplifierChain;
use crate::sim::tuner::Tuner;

pub enum EngineMessage {
    SetAmpChain(Box<AmplifierChain>),
    StartRecording(Recorder),
    StopRecording(),
    SetIrCabinet(Option<String>),
    SetIrBypass(bool),
    SetIrGain(f32),
    SetTunerEnabled(bool),
}

pub struct Engine {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// IR Cabinet processor
    ir_cabinet: Option<IrCabinet>,
    /// Channel for updating the amplifier chain.
    rx_updates: Receiver<EngineMessage>,
    samplers: Samplers,
    tuner: Tuner,
    recorder: Option<Recorder>,
}

impl Engine {
    pub fn new(
        rx_updates: Receiver<EngineMessage>,
        oversample_factor: f64,
        tuner: Tuner,
        buffer_size: usize,
        sample_rate: usize,
    ) -> Result<Self> {
        let samplers =
            Samplers::new(buffer_size, oversample_factor).context("failed to create samplers")?;

        let ir_cabinet = match IrCabinet::new(Path::new("./impulse_responses"), sample_rate) {
            Ok(cab) => {
                debug!("IR Cabinet loaded successfully");
                Some(cab)
            }
            Err(e) => {
                warn!("Failed to load IR Cabinet: {}", e);
                None
            }
        };

        Ok(Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            rx_updates,
            samplers,
            tuner,
            recorder: None,
        })
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        self.handle_messages();

        if self.tuner.is_enabled() {
            self.tuner.process(input);
            output.fill(0.0);
            return Ok(());
        }

        self.samplers.copy_input(input);

        let upsampled = self.samplers.upsample()?;

        let chain = self.chain.as_mut();
        for s in upsampled.iter_mut() {
            *s = chain.process(*s);
        }

        let downsampled = self.samplers.downsample()?;

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(downsampled);
        }

        output[..downsampled.len()].copy_from_slice(downsampled);

        #[allow(clippy::collapsible_if)]
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.record_block(downsampled)?;
        }

        Ok(())
    }

    pub fn update_buffer_size(&mut self, new_size: usize) -> Result<()> {
        self.samplers.resize_buffers(new_size)
    }

    pub fn handle_messages(&mut self) {
        if let Ok(message) = self.rx_updates.try_recv() {
            match message {
                EngineMessage::SetAmpChain(chain) => {
                    self.chain = chain;
                    debug!("Received new amplifier chain");
                }
                EngineMessage::SetIrCabinet(ir_name) => {
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
                EngineMessage::SetIrBypass(bypass) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_bypass(bypass);
                        debug!("IR Cabinet bypass: {}", bypass);
                    }
                }
                EngineMessage::SetIrGain(gain) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_gain(gain);
                        debug!("IR Cabinet gain: {}", gain);
                    }
                }
                EngineMessage::SetTunerEnabled(enabled) => {
                    self.tuner.set_enabled(enabled);
                }
                EngineMessage::StartRecording(recorder) => {
                    if self.recorder.is_some() {
                        debug!("Recorder already active, ignoring start request");
                        return;
                    }

                    debug!("Recorder updated");
                    self.recorder = Some(recorder);
                }
                EngineMessage::StopRecording() => {
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

impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(recorder) = self.recorder.take() {
            debug!("Finalizing recorder on processor drop");
            if let Err(e) = recorder.stop() {
                error!("Failed to stop recorder: {e}");
            }
        }
    }
}
