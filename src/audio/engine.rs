use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use log::{debug, error};

use crate::audio::peak_meter::PeakMeter;
use crate::audio::recorder::Recorder;
use crate::audio::samplers::Samplers;
use crate::ir::cabinet::IrCabinet;
use crate::metronome::Metronome;
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
    engine_receiver: Receiver<EngineMessage>,
    samplers: Samplers,
    tuner: Tuner,
    recorder: Option<Recorder>,
    peak_meter: PeakMeter,
    metronome: Metronome,
}

pub struct EngineHandle {
    engine_sender: Sender<EngineMessage>,
}

impl Engine {
    pub fn new(
        tuner: Tuner,
        samplers: Samplers,
        ir_cabinet: Option<IrCabinet>,
        peak_meter: PeakMeter,
        metronome: Metronome,
    ) -> Result<(Self, EngineHandle)> {
        let (engine_sender, engine_receiver) = bounded::<EngineMessage>(10);

        Ok((
            Self {
                chain: Box::new(AmplifierChain::new()),
                ir_cabinet,
                engine_receiver,
                samplers,
                tuner,
                recorder: None,
                peak_meter,
                metronome,
            },
            EngineHandle { engine_sender },
        ))
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        self.handle_messages();

        if self.tuner.is_enabled() {
            self.tuner.process(input);
            output.fill(0.0);
            return Ok(());
        }

        if self.samplers.get_oversample_factor() == 1.0 {
            self.process_without_upsampling(input, output)?;
        } else {
            self.process_with_upsampling(input, output)?;
        }

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(output);
        }

        self.peak_meter.process(output);

        if let Some(recorder) = self.recorder.as_mut() {
            recorder.record_block(output)?;
        }

        Ok(())
    }

    fn process_without_upsampling(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if input.len() != output.len() {
            return Err(anyhow::anyhow!(
                "input and output buffer size mismatch: input {}, output {}",
                input.len(),
                output.len()
            ));
        }

        let chain = self.chain.as_mut();
        for (i, &sample) in input.iter().enumerate() {
            output[i] = chain.process(sample);
        }

        Ok(())
    }

    fn process_with_upsampling(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        self.samplers.copy_input(input)?;

        let upsampled = self.samplers.upsample()?;

        let chain = self.chain.as_mut();
        for s in upsampled.iter_mut() {
            *s = chain.process(*s);
        }

        let downsampled = self.samplers.downsample()?;

        output[..downsampled.len()].copy_from_slice(downsampled);

        Ok(())
    }

    //need to process metronome seperately
    pub fn process_metronome(&mut self, output: &mut [f32]) {
        if self.metronome.is_enabled() {
            self.metronome.process_block(output);
        }
    }
    pub fn update_buffer_size(&mut self, new_size: usize) -> Result<()> {
        self.samplers.resize_buffers(new_size)
    }

    pub fn handle_messages(&mut self) {
        if let Ok(message) = self.engine_receiver.try_recv() {
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

impl EngineHandle {
    pub fn send(&self, message: EngineMessage) {
        self.engine_sender.try_send(message).unwrap_or_else(|e| {
            error!("Failed to send new amplifier chain: {e}");
        });
    }

    pub fn set_ir_cabinet(&self, ir_name: Option<String>) {
        let update = EngineMessage::SetIrCabinet(ir_name);
        self.send(update);
    }

    pub fn set_ir_bypass(&self, bypass: bool) {
        let update = EngineMessage::SetIrBypass(bypass);
        self.send(update);
    }

    pub fn set_ir_gain(&self, gain: f32) {
        let update = EngineMessage::SetIrGain(gain);
        self.send(update);
    }

    pub fn set_tuner_enabled(&self, enabled: bool) {
        let update = EngineMessage::SetTunerEnabled(enabled);
        self.send(update);
    }

    pub fn set_amp_chain(&self, new_chain: AmplifierChain) {
        let update = EngineMessage::SetAmpChain(Box::new(new_chain));
        self.send(update);
    }

    pub fn start_recording(&self, sample_rate: usize, output_dir: &str) -> Result<()> {
        let recorder = Recorder::new(sample_rate as u32, output_dir)?;

        let update = EngineMessage::StartRecording(recorder);
        self.send(update);

        Ok(())
    }

    pub fn stop_recording(&self) {
        let update = EngineMessage::StopRecording();
        self.send(update);
    }
}
