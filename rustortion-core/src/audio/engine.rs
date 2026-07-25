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

/// Latency, in frames, that [`PitchShifter`] adds when installed.
///
/// It is a 2048-point STFT phase vocoder with a 75% overlap, so the group delay
/// is one full FFT frame. Measured, not assumed: an impulse fed through
/// `PitchShifter::new(0.0)` comes out with its energy centroid at sample 2048
/// (see the `pitch_shifter_latency_matches_constant` test), which is ~42.7 ms at
/// 48 kHz. Must be kept in step with `pitch_shifter::FFT_SIZE`; the test fails
/// if it drifts.
pub const PITCH_SHIFTER_LATENCY_FRAMES: usize = 2048;

/// Level, relative to the peak, at which a decaying tail is treated as over.
/// -60 dB is the usual RT60 convention.
const TAIL_FLOOR: f32 = 0.001;

/// Freeverb comb feedback is `room_size * SCALE_ROOM + OFFSET_ROOM`, and the
/// longest comb is `COMB_DELAYS[7]` frames at `REFERENCE_SAMPLE_RATE`. These
/// mirror the private constants in `amp::stages::reverb`; `reverb_tail_seconds`
/// is the only consumer and the `reverb_tail_*` tests pin the result, so a
/// drift in the reverb's tuning shows up as a test failure rather than silently
/// wrong tail reporting.
const REVERB_SCALE_ROOM: f32 = 0.28;
const REVERB_OFFSET_ROOM: f32 = 0.7;
const REVERB_LONGEST_COMB_SECONDS: f32 = 1617.0 / 44100.0;

/// Seconds a delay line keeps ringing after its input stops: the time for the
/// feedback path to decay to [`TAIL_FLOOR`]. Zero when the stage is inaudible
/// (no wet signal) or has no delay time; one repeat when there is no feedback.
fn delay_tail_seconds(delay_ms: f32, feedback: f32, mix: f32) -> f32 {
    if mix <= 0.0 || delay_ms <= 0.0 {
        return 0.0;
    }
    let feedback = feedback.clamp(0.0, 0.999);
    let repeats = if feedback <= 0.0 {
        1.0
    } else {
        TAIL_FLOOR.log(feedback)
    };
    delay_ms * 0.001 * repeats
}

/// Seconds a Freeverb tank keeps ringing: RT60 of its longest lowpass-feedback
/// comb. Damping only shortens this (it is a lowpass inside the feedback loop),
/// so ignoring it errs on the safe side.
fn reverb_tail_seconds(room_size: f32, mix: f32) -> f32 {
    if mix <= 0.0 {
        return 0.0;
    }
    let feedback = room_size
        .clamp(0.0, 1.0)
        .mul_add(REVERB_SCALE_ROOM, REVERB_OFFSET_ROOM);
    REVERB_LONGEST_COMB_SECONDS * TAIL_FLOOR.log(feedback)
}

pub struct PreparedIr {
    pub name: String,
    /// Boxed so it can be swapped into the cabinet on the RT thread without
    /// reallocating, and so the whole `PreparedIr` (old convolver + name) can
    /// be retired off the RT thread in one piece.
    pub convolver: Box<Convolver>,
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
    /// Carries a fully-constructed pitch shifter (built off the RT thread), or
    /// `None` to disable pitch shifting (the `0` semitones bypass case).
    SetPitchShift(Option<Box<PitchShifter>>),
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
    /// Boxed so swapping samplers (sample-rate / buffer changes) on the RT
    /// thread exchanges pointers and retires the old box directly.
    samplers: Box<Samplers>,
    tuner: Option<Tuner>,
    recorder: Option<Recorder>,
    peak_meter: Option<PeakMeter>,
    metronome: Option<Metronome>,
    pitch_shifter: Option<Box<PitchShifter>>,
    input_highpass: Option<Box<dyn Stage>>,
    input_lowpass: Option<Box<dyn Stage>>,
    /// When true, skip tuner, peak meter, recorder, and metronome processing.
    lightweight: bool,
    /// Cached output of [`Engine::recompute_tail`], refreshed whenever a
    /// message that could change the chain, the IR, or a stage parameter is
    /// handled — so the audio thread only ever reads a `f32` field.
    tail_seconds: f32,
    /// Set once, with the triggering block length, when the oversampling FIFO
    /// engages. Drained by [`Engine::take_fifo_engaged_event`] so the caller can
    /// report it off the audio thread.
    fifo_engaged_event: Option<usize>,
    /// Set once, with the block length, on the first block that took the
    /// zero-latency fast path. Drained by [`Engine::take_fast_path_event`].
    fast_path_event: Option<usize>,
    /// Latch for `fast_path_event`, so it is recorded once and not once per
    /// block if the event goes unread.
    fast_path_reported: bool,
    /// Set by the master-bus guard when a non-finite sample was replaced.
    /// Sticky until read via [`Engine::take_nonfinite_seen`]; the RT thread
    /// only ever sets a `bool` here, reporting happens off the audio thread.
    nonfinite_seen: bool,
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
        let (engine_sender, engine_receiver) = bounded::<EngineMessage>(128);

        let mut engine = Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            engine_receiver,
            rt_drop,
            samplers: Box::new(samplers),
            tuner: Some(tuner),
            recorder: None,
            peak_meter: Some(peak_meter),
            metronome: Some(metronome),
            pitch_shifter: None,
            input_highpass: None,
            input_lowpass: None,
            lightweight: false,
            tail_seconds: 0.0,
            fifo_engaged_event: None,
            fast_path_event: None,
            fast_path_reported: false,
            nonfinite_seen: false,
        };
        engine.recompute_tail();

        Ok((engine, EngineHandle { engine_sender }))
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
        // Plugin hosts hand out blocks of any length up to `max_buffer_size`
        // (short blocks happen at loop boundaries, transport start, and during
        // offline bounce), so the samplers must tolerate that.
        let samplers =
            Samplers::new_variable_block(max_buffer_size, oversample_factor, sample_rate)?;
        let (rt_drop_handle, rt_drop_rx) = RtDropHandle::new();
        let (engine_sender, engine_receiver) = bounded::<EngineMessage>(128);

        let mut engine = Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            engine_receiver,
            rt_drop: rt_drop_handle,
            samplers: Box::new(samplers),
            tuner: None,
            recorder: None,
            peak_meter: None,
            metronome: None,
            pitch_shifter: None,
            input_highpass: None,
            input_lowpass: None,
            lightweight: true,
            tail_seconds: 0.0,
            fifo_engaged_event: None,
            fast_path_event: None,
            fast_path_reported: false,
            nonfinite_seen: false,
        };
        engine.recompute_tail();

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

        self.sanitize_master_bus(output);

        if let Some(ref mut peak_meter) = self.peak_meter {
            peak_meter.process(output);
        }

        if !self.lightweight
            && let Some(recorder) = self.recorder.as_mut()
        {
            recorder.record_block(output);
        }

        Ok(())
    }

    /// Master-bus safety net: replace any non-finite sample with silence.
    ///
    /// A NaN or infinity produced anywhere upstream (feedback paths in the
    /// delay, reverb, or EQ at extreme settings are the usual sources) would
    /// otherwise reach the output *and* stay wedged in that stage's feedback
    /// state forever, since every subsequent sample it touches becomes NaN too.
    /// This does not fix the source — it only stops the damage at the output,
    /// and records that it happened so a non-RT context can report it once.
    ///
    /// One branch per sample on an already-branchy master bus; no logging, no
    /// allocation, nothing that is unsafe on the audio thread.
    fn sanitize_master_bus(&mut self, output: &mut [f32]) {
        let mut seen = false;
        for s in output.iter_mut() {
            if !s.is_finite() {
                *s = 0.0;
                seen = true;
            }
        }
        self.nonfinite_seen |= seen;
    }

    /// The fixed number of frames the oversampling resamplers work in. Host
    /// blocks that are an exact multiple of this take the zero-latency path.
    pub fn resampler_chunk_frames(&self) -> usize {
        self.samplers.chunk_frames()
    }

    /// Whether the oversampling FIFO has engaged. False means every block so
    /// far took the zero-latency fast path.
    pub fn fifo_engaged(&self) -> bool {
        self.samplers.fifo_engaged()
    }

    /// Read and clear the "oversampling FIFO just engaged" event, yielding the
    /// block length that triggered it. Fires at most once per activation
    /// (engagement is sticky). Returning the number instead of logging keeps
    /// all formatting off the audio thread.
    pub const fn take_fifo_engaged_event(&mut self) -> Option<usize> {
        self.fifo_engaged_event.take()
    }

    /// Read and clear the "first block took the fast path" event, yielding that
    /// block's length. Fires at most once per activation, so a session that
    /// never engages the FIFO still leaves evidence that the fast path was
    /// taken.
    pub const fn take_fast_path_event(&mut self) -> Option<usize> {
        self.fast_path_event.take()
    }

    /// Read and clear the non-finite-output flag set by the master-bus guard.
    /// Cheap enough to call from the audio thread; the caller is expected to do
    /// the actual reporting elsewhere.
    pub const fn take_nonfinite_seen(&mut self) -> bool {
        let seen = self.nonfinite_seen;
        self.nonfinite_seen = false;
        seen
    }

    /// How long, in seconds, the engine keeps producing sound after its input
    /// goes silent — the "tail" a plugin host needs in order to not cut off the
    /// last note when the transport stops.
    ///
    /// This is a cached value, recomputed only when a message that could change
    /// the chain, a stage parameter, or the IR is handled, so reading it on the
    /// audio thread is a field load. `0.0` means nothing in the current chain
    /// rings, and the host is free to idle the plugin entirely.
    pub const fn tail_seconds(&self) -> f32 {
        self.tail_seconds
    }

    /// Recompute [`Engine::tail_seconds`] from what is *actually* in the chain
    /// right now: the max over the active tail sources.
    ///
    /// Deliberately not derived from the IR alone — presets frequently run with
    /// no cab loaded, and a chain with no IR can still ring for seconds via the
    /// delay or reverb, while a clean chain with neither rings not at all.
    ///
    /// Stages are identified by the parameters they expose (`delay_time` is
    /// unique to the delay stage, `room_size` to the reverb), since `dyn Stage`
    /// carries no type tag and the chain exposes no iterator. `get_parameter`
    /// returns `None` only for an out-of-range index, which is what ends the
    /// scan. Nothing here allocates, locks, or does I/O, but it is off the
    /// per-block path regardless.
    fn recompute_tail(&mut self) {
        let mut tail: f32 = 0.0;

        // The cab only rings while an IR is actually loaded and audible. Its
        // contribution is bounded by the truncation limit rather than the real
        // IR length, which the cabinet does not expose — the difference is tens
        // of milliseconds against delay/reverb tails measured in seconds.
        if self
            .ir_cabinet
            .as_ref()
            .is_some_and(|cab| cab.has_ir() && !cab.is_bypassed())
        {
            #[allow(clippy::cast_precision_loss)]
            let ir_seconds = crate::ir::cabinet::DEFAULT_MAX_IR_MS as f32 * 0.001;
            tail = tail.max(ir_seconds);
        }

        let mut idx = 0;
        while let Some(delay_time) = self.chain.get_parameter(idx, "delay_time") {
            if let Ok(delay_ms) = delay_time {
                let feedback = self.stage_param(idx, "feedback", 0.0);
                let mix = self.stage_param(idx, "mix", 1.0);
                tail = tail.max(delay_tail_seconds(delay_ms, feedback, mix));
            } else if let Some(Ok(room_size)) = self.chain.get_parameter(idx, "room_size") {
                let mix = self.stage_param(idx, "mix", 1.0);
                tail = tail.max(reverb_tail_seconds(room_size, mix));
            }
            idx += 1;
        }

        self.tail_seconds = tail;
    }

    /// Read one parameter off a stage, falling back to `default` when the stage
    /// does not have it.
    fn stage_param(&self, idx: usize, name: &str, default: f32) -> f32 {
        self.chain
            .get_parameter(idx, name)
            .and_then(Result::ok)
            .unwrap_or(default)
    }

    /// Latency in frames the engine currently adds, at the host sample rate.
    ///
    /// Sum of the oversampling path (resampler group delay + variable-block
    /// FIFO pre-fill, both zero at 1x) and the pitch shifter's STFT delay
    /// (zero when no shifter is installed, which is the `0 semitones` case).
    /// Changes when the user changes oversampling or pitch shift, so callers
    /// must re-read it and re-report to the host.
    pub fn latency_frames(&self) -> usize {
        self.samplers.latency_frames()
            + self
                .pitch_shifter
                .as_ref()
                .map_or(0, |_| PITCH_SHIFTER_LATENCY_FRAMES)
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
        self.chain.as_mut().process_block(output);

        Ok(())
    }

    fn process_with_upsampling(&mut self, output: &mut [f32]) -> Result<()> {
        if self.samplers.is_variable_block() {
            return self.process_variable_block(output);
        }

        self.samplers.copy_input(output)?;

        let upsampled = self.samplers.upsample()?;

        self.chain.as_mut().process_block(upsampled);

        let downsampled = self.samplers.downsample()?;

        output[..downsampled.len()].copy_from_slice(downsampled);

        Ok(())
    }

    /// Oversampled processing for a host that may vary its block size.
    ///
    /// Two modes, and the cheap one is the default. While the FIFO is
    /// disengaged, a block whose length is an exact multiple of the resampler
    /// chunk is run straight through as `len / chunk` back-to-back chunks: no
    /// queueing, no prefill, and the only latency is the resamplers' own group
    /// delay. Every standard DAW buffer size (64 … 2048) is a multiple of the
    /// 64-frame chunk, so a fixed-block session stays here forever.
    ///
    /// Anything else — a short final block, a ragged length, an odd
    /// `max_buffer_size` — engages the FIFO permanently and takes the buffered
    /// path from then on. The switch costs one re-reported latency figure and a
    /// one-time discontinuity as the prefill silence is inserted.
    fn process_variable_block(&mut self, output: &mut [f32]) -> Result<()> {
        // A zero-length block is a no-op, not a ragged one. Falling through
        // would engage the FIFO permanently — and its prefill latency — for a
        // block that carries no audio at all.
        if output.is_empty() {
            return Ok(());
        }

        let chunk = self.samplers.chunk_frames();

        if !self.samplers.fifo_engaged() {
            if output.len().is_multiple_of(chunk) {
                for offset in (0..output.len()).step_by(chunk) {
                    self.samplers.copy_input(&output[offset..offset + chunk])?;
                    let upsampled = self.samplers.upsample()?;
                    self.chain.as_mut().process_block(upsampled);
                    let downsampled = self.samplers.downsample()?;
                    if downsampled.len() != chunk {
                        return Err(anyhow::anyhow!(
                            "downsampler produced {} frames, expected {chunk}",
                            downsampled.len()
                        ));
                    }
                    output[offset..offset + chunk].copy_from_slice(downsampled);
                }
                if !self.fast_path_reported {
                    self.fast_path_reported = true;
                    self.fast_path_event = Some(output.len());
                }
                return Ok(());
            }

            self.samplers.engage_fifo();
            self.fifo_engaged_event = Some(output.len());
        }

        // Queue the block, run whole chunks through up -> chain -> down, and
        // serve this block from the output FIFO (see `samplers::BlockFifo`).
        self.samplers.fifo_push_input(output);
        while self.samplers.fifo_load_chunk() {
            let upsampled = self.samplers.upsample()?;
            self.chain.as_mut().process_block(upsampled);
            self.samplers.downsample_into_fifo()?;
        }
        self.samplers.fifo_pop_output(output);

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
        let mut handled_any = false;
        while let Ok(message) = self.engine_receiver.try_recv() {
            handled_any = true;
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
                    if let Some(rejected) = self.chain.insert_stage(idx, stage) {
                        // Chain is at its reserved capacity. Retire the rejected
                        // stage off the RT thread rather than dropping (freeing)
                        // it here. The UI caps stage count, so this is a backstop.
                        self.rt_drop.retire(rejected);
                    } else {
                        debug!("Added stage at index {idx}");
                    }
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
                    // Retire the previous filters off the RT thread instead of
                    // dropping them here on direct assignment.
                    if let Some(old) = std::mem::replace(&mut self.input_highpass, hp) {
                        self.rt_drop.retire(old);
                    }
                    if let Some(old) = std::mem::replace(&mut self.input_lowpass, lp) {
                        self.rt_drop.retire(old);
                    }
                    debug!("Updated input filters");
                }
                EngineMessage::SwapIrConvolver(mut prepared) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        debug!("IR convolver swapped: {}", prepared.name);
                        // Swap the new convolver in; `prepared` is left holding
                        // the old convolver. Retire the whole `PreparedIr` (old
                        // convolver + name `String`) off the RT thread so
                        // nothing deallocates here.
                        cab.swap_convolver(&mut prepared.convolver);
                    }
                    self.rt_drop.retire(prepared);
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
                EngineMessage::SetPitchShift(shifter) => {
                    self.handle_pitch_shift(shifter);
                }
                EngineMessage::SetSamplers(new_samplers) => {
                    let old = std::mem::replace(&mut self.samplers, new_samplers);
                    self.rt_drop.retire(old);
                    debug!("Samplers swapped");
                }
            }
        }

        // Every mutation of the chain, its parameters, and the IR arrives as a
        // message, so this is the one place the reported tail can go stale.
        // Messages are rare (user edits), blocks are not — hence recompute here
        // and cache, rather than scanning the chain every block.
        if handled_any {
            self.recompute_tail();
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

    fn handle_pitch_shift(&mut self, shifter: Option<Box<PitchShifter>>) {
        // The shifter (if any) is constructed off the RT thread in
        // `EngineHandle::set_pitch_shift`; here we just swap it in and retire
        // the previous one off the RT thread.
        let old = std::mem::replace(&mut self.pitch_shifter, shifter);
        if let Some(old) = old {
            self.rt_drop.retire(old);
        }
        debug!("Pitch shifter updated");
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
        // Construct the pitch shifter here (GUI thread) so the RT thread never
        // allocates its FFT plans / scratch buffers. `0` semitones == bypass.
        let shifter = if semitones == 0 {
            None
        } else {
            Some(Box::new(PitchShifter::new(semitones as f32)))
        };
        self.send(EngineMessage::SetPitchShift(shifter));
    }

    pub fn set_stage_bypassed(&self, idx: usize, bypassed: bool) {
        self.send(EngineMessage::SetStageBypassed(idx, bypassed));
    }

    pub fn set_input_filters(&self, hp: Option<Box<dyn Stage>>, lp: Option<Box<dyn Stage>>) {
        let update = EngineMessage::SetInputFilters(hp, lp);
        self.send(update);
    }

    pub fn start_recording(
        &self,
        sample_rate: usize,
        output_dir: &str,
        max_block_samples: usize,
    ) -> Result<()> {
        let recorder = Recorder::new(sample_rate as u32, output_dir, max_block_samples)?;

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
