use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use log::{debug, error};

use crate::amp::chain::AmplifierChain;
use crate::amp::stages::Stage;
use crate::audio::peak_meter::PeakMeter;
use crate::audio::pitch_shifter::PitchShifter;
use crate::audio::recorder::Recorder;
use crate::audio::rt_drop::RtDropHandle;
use crate::audio::samplers::Samplers;
use crate::ir::cabinet::IrCabinet;
use crate::ir::convolver::Convolver;
use crate::metronome::Metronome;
use crate::tuner::Tuner;

pub struct PreparedIr {
    pub name: String,
    pub convolver: Convolver,
}

pub enum EngineMessage {
    SetAmpChain(Box<AmplifierChain>),
    SetInputFilters(Option<Box<dyn Stage>>, Option<Box<dyn Stage>>),
    SetParameter(usize, &'static str, f32),
    ReplaceStage(usize, Box<dyn Stage>),
    AddStage(usize, Box<dyn Stage>),
    RemoveStage(usize),
    SwapStages(usize, usize),
    StartRecording(Recorder),
    StopRecording,
    SwapIrConvolver(Box<PreparedIr>),
    ClearIr,
    SetIrBypass(bool),
    SetIrGain(f32),
    SetTunerEnabled(bool),
    SetPitchShift(i32),
    SetStageBypassed(usize, bool),
    SetSamplers(Box<Samplers>),
}

pub struct Engine {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// IR Cabinet processor
    ir_cabinet: Option<IrCabinet>,
    /// Channel for updating the amplifier chain.
    engine_receiver: Receiver<EngineMessage>,
    /// Handle for sending arbitrary objects off the RT thread for deallocation.
    rt_drop: RtDropHandle,
    samplers: Samplers,
    tuner: Option<Tuner>,
    recorder: Option<Recorder>,
    peak_meter: Option<PeakMeter>,
    metronome: Option<Metronome>,
    pitch_shifter: Option<PitchShifter>,
    input_highpass: Option<Box<dyn Stage>>,
    input_lowpass: Option<Box<dyn Stage>>,
    /// When true, skip tuner, peak meter, recorder, and metronome processing.
    lightweight: bool,
}

#[derive(Clone)]
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
        rt_drop: RtDropHandle,
    ) -> Result<(Self, EngineHandle)> {
        let (engine_sender, engine_receiver) = bounded::<EngineMessage>(32);

        Ok((
            Self {
                chain: Box::new(AmplifierChain::new()),
                ir_cabinet,
                engine_receiver,
                rt_drop,
                samplers,
                tuner: Some(tuner),
                recorder: None,
                peak_meter: Some(peak_meter),
                metronome: Some(metronome),
                pitch_shifter: None,
                input_highpass: None,
                input_lowpass: None,
                lightweight: false,
            },
            EngineHandle { engine_sender },
        ))
    }

    /// Create an engine for plugin use (no JACK, no recorder, no file I/O).
    /// Returns `(Engine, EngineHandle, RtDropReceiver)`.
    /// The caller should spawn a thread to run `rt_drop_rx.run()`.
    pub fn new_for_plugin(
        sample_rate: usize,
        max_buffer_size: usize,
        ir_cabinet: Option<IrCabinet>,
        oversample_factor: f64,
    ) -> Result<(Self, EngineHandle, crate::audio::rt_drop::RtDropReceiver)> {
        let samplers = Samplers::new(max_buffer_size, oversample_factor, sample_rate)?;
        let (rt_drop_handle, rt_drop_rx) = RtDropHandle::new();
        let (engine_sender, engine_receiver) = bounded::<EngineMessage>(32);

        let engine = Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            engine_receiver,
            rt_drop: rt_drop_handle,
            samplers,
            tuner: None,
            recorder: None,
            peak_meter: None,
            metronome: None,
            pitch_shifter: None,
            input_highpass: None,
            input_lowpass: None,
            lightweight: true,
        };

        Ok((engine, EngineHandle { engine_sender }, rt_drop_rx))
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if input.len() != output.len() {
            return Err(anyhow::anyhow!(
                "input and output buffer size mismatch: input {}, output {}",
                input.len(),
                output.len()
            ));
        }

        self.handle_messages();

        if let Some(ref mut tuner) = self.tuner
            && tuner.is_enabled()
        {
            tuner.process(input);
            output.fill(0.0);
            return Ok(());
        }

        // Apply input filters in-place via output buffer to avoid allocation.
        // Skip copy when input and output alias (same base pointer).
        if !std::ptr::eq(input.as_ptr(), output.as_ptr()) {
            output[..input.len()].copy_from_slice(input);
        }
        self.apply_input_filters(&mut output[..input.len()]);

        if self.samplers.get_oversample_factor() == 1.0 {
            self.process_without_upsampling(output)?;
        } else {
            self.process_with_upsampling(output)?;
        }

        if let Some(ref mut shifter) = self.pitch_shifter {
            shifter.process_block(output);
        }

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(output);
        }

        if let Some(ref mut peak_meter) = self.peak_meter {
            peak_meter.process(output);
        }

        if !self.lightweight
            && let Some(recorder) = self.recorder.as_mut()
        {
            recorder.record_block(output)?;
        }

        Ok(())
    }

    fn apply_input_filters(&mut self, buf: &mut [f32]) {
        if let Some(ref mut hp) = self.input_highpass {
            for s in buf.iter_mut() {
                *s = hp.process(*s);
            }
        }
        if let Some(ref mut lp) = self.input_lowpass {
            for s in buf.iter_mut() {
                *s = lp.process(*s);
            }
        }
    }

    fn process_without_upsampling(&mut self, output: &mut [f32]) -> Result<()> {
        let chain = self.chain.as_mut();
        for s in output.iter_mut() {
            *s = chain.process(*s);
        }

        Ok(())
    }

    fn process_with_upsampling(&mut self, output: &mut [f32]) -> Result<()> {
        self.samplers.copy_input(output)?;

        let upsampled = self.samplers.upsample()?;

        let chain = self.chain.as_mut();
        for s in upsampled.iter_mut() {
            *s = chain.process(*s);
        }

        let downsampled = self.samplers.downsample()?;

        output[..downsampled.len()].copy_from_slice(downsampled);

        Ok(())
    }

    //need to process metronome separately
    pub fn process_metronome(&mut self, output: &mut [f32]) -> bool {
        if let Some(ref mut metronome) = self.metronome
            && metronome.is_enabled()
        {
            metronome.process_block(output);
            return true;
        }

        false
    }
    pub fn update_buffer_size(&mut self, new_size: usize) -> Result<()> {
        self.samplers.resize_buffers(new_size)
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn handle_messages(&mut self) {
        while let Ok(message) = self.engine_receiver.try_recv() {
            match message {
                EngineMessage::SetAmpChain(new_chain) => {
                    let old = std::mem::replace(&mut self.chain, new_chain);
                    self.rt_drop.retire(old);
                    debug!("Received new amplifier chain");
                }
                EngineMessage::SetParameter(idx, name, value) => {
                    if let Some(result) = self.chain.set_parameter(idx, name, value) {
                        if let Err(e) = result {
                            error!("Failed to set parameter '{name}' on stage {idx}: {e}");
                        }
                    } else {
                        error!("SetParameter: stage index {idx} out of bounds");
                    }
                }
                EngineMessage::ReplaceStage(idx, new_stage) => {
                    if let Some(old) = self.chain.replace_stage(idx, new_stage) {
                        self.rt_drop.retire(old);
                        debug!("Replaced stage at index {idx}");
                    } else {
                        error!("ReplaceStage: stage index {idx} out of bounds");
                    }
                }
                EngineMessage::AddStage(idx, stage) => {
                    self.chain.insert_stage(idx, stage);
                    debug!("Added stage at index {idx}");
                }
                EngineMessage::RemoveStage(idx) => {
                    if let Some(old) = self.chain.remove_stage(idx) {
                        self.rt_drop.retire(old);
                        debug!("Removed stage at index {idx}");
                    } else {
                        error!("RemoveStage: stage index {idx} out of bounds");
                    }
                }
                EngineMessage::SwapStages(a, b) => {
                    self.chain.swap_stages(a, b);
                    debug!("Swapped stages {a} and {b}");
                }
                EngineMessage::SetStageBypassed(idx, bypassed) => {
                    if self.chain.set_bypassed(idx, bypassed) {
                        debug!("Stage {idx} bypass: {bypassed}");
                    } else {
                        error!("SetStageBypassed: stage index {idx} out of bounds");
                    }
                }
                EngineMessage::SetInputFilters(hp, lp) => {
                    self.input_highpass = hp;
                    self.input_lowpass = lp;
                    debug!("Updated input filters");
                }
                EngineMessage::SwapIrConvolver(prepared) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        debug!("IR convolver swapped: {}", prepared.name);
                        let old = cab.swap_convolver(prepared.convolver);
                        self.rt_drop.retire(Box::new(old));
                    }
                }
                EngineMessage::ClearIr => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.clear_convolver();
                        debug!("IR cleared");
                    }
                }
                EngineMessage::SetIrBypass(bypass) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_bypass(bypass);
                        debug!("IR Cabinet bypass: {bypass}");
                    }
                }
                EngineMessage::SetIrGain(gain) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_gain(gain);
                        debug!("IR Cabinet gain: {gain}");
                    }
                }
                EngineMessage::SetTunerEnabled(enabled) => {
                    if let Some(ref mut tuner) = self.tuner {
                        tuner.set_enabled(enabled);
                    }
                }
                EngineMessage::StartRecording(recorder) => {
                    self.handle_start_recording(recorder);
                }
                EngineMessage::StopRecording => {
                    self.handle_stop_recording();
                }
                EngineMessage::SetPitchShift(semitones) => {
                    self.handle_pitch_shift(semitones);
                }
                EngineMessage::SetSamplers(new_samplers) => {
                    let old = std::mem::replace(&mut self.samplers, *new_samplers);
                    self.rt_drop.retire(Box::new(old));
                    debug!("Samplers swapped");
                }
            }
        }
    }

    fn handle_start_recording(&mut self, recorder: Recorder) {
        if self.recorder.is_some() {
            debug!("Recorder already active, ignoring start request");
            return;
        }

        debug!("Recorder updated");
        self.recorder = Some(recorder);
    }

    fn handle_stop_recording(&mut self) {
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

    fn handle_pitch_shift(&mut self, semitones: i32) {
        if semitones == 0 {
            self.pitch_shifter = None;
            debug!("Pitch shift disabled (bypass)");
        } else if let Some(ref mut shifter) = self.pitch_shifter {
            shifter.set_semitones(semitones as f32);
            debug!("Pitch shift set to {semitones} semitones");
        } else {
            self.pitch_shifter = Some(PitchShifter::new(semitones as f32));
            debug!("Pitch shift set to {semitones} semitones");
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
            error!("Failed to send engine message: {e}");
        });
    }

    pub fn swap_ir_convolver(&self, prepared: PreparedIr) {
        let update = EngineMessage::SwapIrConvolver(Box::new(prepared));
        self.send(update);
    }

    pub fn clear_ir(&self) {
        self.send(EngineMessage::ClearIr);
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

    pub fn set_parameter(&self, stage_idx: usize, name: &'static str, value: f32) {
        self.send(EngineMessage::SetParameter(stage_idx, name, value));
    }

    pub fn replace_stage(&self, idx: usize, stage: Box<dyn Stage>) {
        self.send(EngineMessage::ReplaceStage(idx, stage));
    }

    pub fn add_stage(&self, idx: usize, stage: Box<dyn Stage>) {
        self.send(EngineMessage::AddStage(idx, stage));
    }

    pub fn remove_stage(&self, idx: usize) {
        self.send(EngineMessage::RemoveStage(idx));
    }

    pub fn swap_stages(&self, a: usize, b: usize) {
        self.send(EngineMessage::SwapStages(a, b));
    }

    pub fn set_amp_chain(&self, new_chain: AmplifierChain) {
        let update = EngineMessage::SetAmpChain(Box::new(new_chain));
        self.send(update);
    }

    pub fn set_pitch_shift(&self, semitones: i32) {
        let update = EngineMessage::SetPitchShift(semitones);
        self.send(update);
    }

    pub fn set_stage_bypassed(&self, idx: usize, bypassed: bool) {
        self.send(EngineMessage::SetStageBypassed(idx, bypassed));
    }

    pub fn set_input_filters(&self, hp: Option<Box<dyn Stage>>, lp: Option<Box<dyn Stage>>) {
        let update = EngineMessage::SetInputFilters(hp, lp);
        self.send(update);
    }

    pub fn start_recording(&self, sample_rate: usize, output_dir: &str) -> Result<()> {
        let recorder = Recorder::new(sample_rate as u32, output_dir)?;

        let update = EngineMessage::StartRecording(recorder);
        self.send(update);

        Ok(())
    }

    pub fn stop_recording(&self) {
        let update = EngineMessage::StopRecording;
        self.send(update);
    }

    pub fn set_samplers(&self, samplers: Samplers) {
        self.send(EngineMessage::SetSamplers(Box::new(samplers)));
    }
}
