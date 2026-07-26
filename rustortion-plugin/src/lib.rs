use nih_plug::prelude::*;
use rustortion_core::audio::engine::{Engine, EngineHandle};
use rustortion_core::ir::loader::IrLoader;
use rustortion_core::preset::stage_config::StageConfig;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

mod backend;
mod editor;
mod factory;
mod ir_helper;
pub mod params;

use params::RustortionParams;

/// Directory the plugin loads user-provided `.nam` models from:
/// `~/.config/rustortion/nam`. Shared by the init-time loader and the backend's
/// rescan so the two can never drift to different paths.
#[must_use]
pub fn user_nam_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("rustortion")
        .join("nam")
}

enum PluginTask {
    LoadPreset(String),
    /// Combined task: create new samplers at the given factor, then reload the
    /// preset so time-based stages are rebuilt at the correct effective rate.
    ChangeOversamplingAndReload {
        factor: u32,
        preset_name: Option<String>,
    },
    /// Report a problem detected on the audio thread. The audio thread only
    /// hands over a `&'static str` (no formatting, no allocation); the logging
    /// itself happens here, off the RT path, and each condition is reported at
    /// most once per activation.
    LogRuntimeIssue(&'static str),
    /// Report which oversampling path the audio thread settled on. Every field
    /// is a plain integer the RT thread already had in hand — no formatting and
    /// no allocation there; the message is assembled in the task executor.
    LogOversamplingMode {
        /// False for the first fast-path block, true when the FIFO engaged.
        engaged: bool,
        /// Host block length that triggered the event. For `engaged`, this is
        /// the length that was not an exact multiple of `chunk_frames`.
        block_len: u32,
        /// Fixed resampler chunk size in frames.
        chunk_frames: u32,
        /// Reported latency before and after the event, in frames.
        latency_before: u32,
        latency_after: u32,
        /// Host sample rate, for rendering the frame counts as milliseconds.
        sample_rate: f32,
    },
}

pub(crate) struct SharedState {
    engine_handle: Mutex<Option<EngineHandle>>,
    ir_loader: Mutex<Option<Arc<IrLoader>>>,
    preset_manager: Mutex<Option<Arc<rustortion_core::preset::Manager>>>,
    sample_rate: AtomicU32,
    max_buffer_size: AtomicU32,
    /// Oversampling factor requested by the GUI (1, 2, 4, 8, 16).
    /// Written by GUI thread, read by process thread to detect changes.
    requested_oversampling: AtomicU32,
    /// Oversampling factor currently applied to the engine's samplers.
    /// Written by background task after samplers are created, read by
    /// `PluginBackend::effective_sample_rate()` so chain rebuilds match
    /// the actual sampler state.
    active_oversampling: AtomicU32,
    /// GUI stage chain — survives editor close/reopen within the same session.
    gui_stages: Mutex<Option<Vec<StageConfig>>>,
}

impl SharedState {
    pub(crate) fn store_gui_stages(&self, stages: &[StageConfig]) {
        if let Ok(mut g) = self.gui_stages.lock() {
            *g = Some(stages.to_vec());
        }
    }

    pub(crate) fn take_gui_stages(&self) -> Option<Vec<StageConfig>> {
        self.gui_stages.lock().ok()?.clone()
    }
}

struct RustortionPlugin {
    params: Arc<RustortionParams>,
    engine: Option<Engine>,
    engine_handle: Option<EngineHandle>,
    rt_drop_thread: Option<std::thread::JoinHandle<()>>,
    sample_rate: f32,
    shared: Arc<SharedState>,
    preset_names: Vec<String>,
    editor_preset_names: Arc<Mutex<Vec<String>>>,
    last_preset_idx: i32,
    last_ir_gain: f32,
    active_oversampling: u32,
    input_buf: Vec<f32>,
    output_buf: Vec<f32>,
    /// Latency last reported to the host, so `set_latency_samples` is only
    /// called when it actually changes (it can restart host playback).
    reported_latency: u32,
    /// One-shot guards so an event on the audio thread is logged once per
    /// activation instead of once per block.
    reported: ReportOnce,
}

/// Bitmask of "already reported" flags. A bitmask rather than a field per
/// event so adding one does not grow the struct, and so the whole set resets in
/// a single store on activation.
#[derive(Default)]
struct ReportOnce(u8);

impl ReportOnce {
    const PROCESS_ERROR: u8 = 1 << 0;
    const NONFINITE: u8 = 1 << 1;
    const FIFO_ENGAGED: u8 = 1 << 2;
    const FAST_PATH: u8 = 1 << 3;
    const OVERSIZED_BLOCK: u8 = 1 << 4;

    /// True the first time `flag` is claimed, false forever after.
    const fn claim(&mut self, flag: u8) -> bool {
        if self.0 & flag == 0 {
            self.0 |= flag;
            true
        } else {
            false
        }
    }

    const fn reset(&mut self) {
        self.0 = 0;
    }
}

impl Default for RustortionPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(RustortionParams::default()),
            engine: None,
            engine_handle: None,
            rt_drop_thread: None,
            sample_rate: 44100.0,
            shared: Arc::new(SharedState {
                engine_handle: Mutex::new(None),
                ir_loader: Mutex::new(None),
                preset_manager: Mutex::new(None),
                sample_rate: AtomicU32::new(0),
                max_buffer_size: AtomicU32::new(0),
                requested_oversampling: AtomicU32::new(1),
                active_oversampling: AtomicU32::new(1),
                gui_stages: Mutex::new(None),
            }),
            preset_names: Vec::new(),
            editor_preset_names: Arc::new(Mutex::new(Vec::new())),
            last_preset_idx: -1,
            last_ir_gain: util::db_to_gain(-20.0),
            active_oversampling: 1, // 1x (no oversampling)
            input_buf: Vec::new(),
            output_buf: Vec::new(),
            reported_latency: 0,
            reported: ReportOnce::default(),
        }
    }
}

/// Tail length at or above which the plugin reports `KeepAlive` instead of a
/// finite tail, in seconds.
///
/// Reached only by a delay at very high feedback or a reverb at a very large
/// room size — settings whose ring is, in musical terms, self-sustaining. A
/// finite tail there would be a lie the host acts on by suspending mid-ring, so
/// this specific case gets `KeepAlive`. Everything below it reports the real
/// computed figure, and a chain with nothing that rings reports `Normal` so the
/// host can genuinely idle the instance.
const KEEP_ALIVE_THRESHOLD_SECONDS: f32 = 60.0;

/// Format and emit the oversampling-mode message. Lives off the audio thread:
/// the RT side only ever hands over the integers below.
fn log_oversampling_mode(
    engaged: bool,
    block_len: u32,
    chunk_frames: u32,
    latency_before: u32,
    latency_after: u32,
    sample_rate: f32,
) {
    let ms = |frames: u32| {
        if sample_rate > 0.0 {
            f64::from(frames) * 1000.0 / f64::from(sample_rate)
        } else {
            0.0
        }
    };
    if engaged {
        nih_log!(
            "Oversampling FIFO engaged: host block of {block_len} frames is not \
                             an exact multiple of the {chunk_frames}-frame resampler chunk. \
                             Latency {latency_before} frames ({:.2} ms) -> {latency_after} frames \
                             ({:.2} ms). This is permanent for the lifetime of this instance.",
            ms(latency_before),
            ms(latency_after),
        );
    } else {
        log::debug!(
            "Oversampling fast path in use: host block of {block_len} frames is \
                             an exact multiple of the {chunk_frames}-frame resampler chunk, so \
                             the FIFO stays disengaged. Latency {latency_after} frames ({:.2} ms).",
            ms(latency_after),
        );
    }
}

/// Zero every output channel from `from` to the end of the block.
///
/// The host copies input into the output buffer before `process()` runs, so
/// any frame we leave untouched is the unprocessed dry guitar at full level —
/// on a high-gain amp sim that is both wrong and loud. Anywhere we decline to
/// process, we silence instead of returning early.
fn silence(buffer: &mut Buffer, from: usize) {
    for ch in buffer.as_slice().iter_mut() {
        if from < ch.len() {
            ch[from..].fill(0.0);
        }
    }
}

/// Sum every input channel into `input_buf` at unity average gain.
fn sum_to_mono(buffer: &Buffer, input_buf: &mut [f32], num_samples: usize) {
    let channel_slices = buffer.as_slice_immutable();
    if channel_slices.is_empty() {
        return;
    }
    #[allow(clippy::cast_precision_loss)] // channel count < 2^24
    let scale = 1.0 / channel_slices.len() as f32;
    for (i, out) in input_buf.iter_mut().enumerate().take(num_samples) {
        let mut sum = 0.0;
        for ch in channel_slices {
            sum += ch[i];
        }
        *out = sum * scale;
    }
}

/// What the audio thread observed about the oversampling path this block.
struct OversamplingEvent {
    engaged_block: Option<usize>,
    fast_path_block: Option<usize>,
    chunk_frames: u32,
    latency_after: u32,
}

fn do_load_preset(
    handle: &EngineHandle,
    manager: Option<&rustortion_core::preset::Manager>,
    ir_loader: Option<&IrLoader>,
    sample_rate: f32,
    oversampling_factor: u32,
    name: &str,
) {
    let Some(manager) = manager else {
        return;
    };
    let Some(preset) = manager.get_preset_by_name(name) else {
        return;
    };

    // Stages in the amp chain run at the oversampled rate
    #[allow(clippy::cast_precision_loss)]
    let effective_sr = sample_rate * oversampling_factor as f32;

    // Build amp chain from preset stages
    let mut chain = rustortion_core::amp::chain::AmplifierChain::new();
    for stage_cfg in &preset.stages {
        chain.add_stage(stage_cfg.to_runtime(effective_sr));
    }
    for (i, stage_cfg) in preset.stages.iter().enumerate() {
        if stage_cfg.bypassed() {
            chain.set_bypassed(i, true);
        }
    }
    handle.set_amp_chain(chain);

    // Set pitch shift
    handle.set_pitch_shift(preset.pitch_shift_semitones);

    // Load IR if specified
    if let Some(ir_name) = &preset.ir_name {
        if let Some(loader) = ir_loader {
            if let Some(bytes) = factory::get_factory_ir(ir_name) {
                ir_helper::load_and_set_ir_from_bytes(handle, loader, ir_name, &bytes, sample_rate);
            } else {
                ir_helper::load_and_set_ir(handle, loader, ir_name, sample_rate);
            }
        }
    } else {
        handle.clear_ir();
    }

    // Set IR gain
    handle.set_ir_gain(preset.ir_gain);

    // Set input filters
    let filters = &preset.input_filters;
    let hp: Option<Box<dyn rustortion_core::amp::stages::Stage>> = if filters.hp_enabled {
        Some(Box::new(
            rustortion_core::amp::stages::filter::FilterStage::new(
                rustortion_core::amp::stages::filter::FilterType::Highpass,
                filters.hp_cutoff,
                sample_rate,
            ),
        ))
    } else {
        None
    };
    let lp: Option<Box<dyn rustortion_core::amp::stages::Stage>> = if filters.lp_enabled {
        Some(Box::new(
            rustortion_core::amp::stages::filter::FilterStage::new(
                rustortion_core::amp::stages::filter::FilterType::Lowpass,
                filters.lp_cutoff,
                sample_rate,
            ),
        ))
    } else {
        None
    };
    handle.set_input_filters(hp, lp);
}

impl RustortionPlugin {
    /// Hand an oversampling-path change to the background thread to log.
    ///
    /// The engine's events are already one-shot; the [`ReportOnce`] latch makes
    /// that belt-and-braces, so a bug upstream can never turn this into a
    /// per-block log. Everything passed is a plain integer — no formatting and
    /// no allocation happens here.
    fn report_oversampling_mode(
        &mut self,
        context: &impl ProcessContext<Self>,
        event: &OversamplingEvent,
    ) {
        let (engaged, block_len) = if let Some(len) = event.engaged_block {
            if !self.reported.claim(ReportOnce::FIFO_ENGAGED) {
                return;
            }
            (true, len)
        } else if let Some(len) = event.fast_path_block {
            if !self.reported.claim(ReportOnce::FAST_PATH) {
                return;
            }
            (false, len)
        } else {
            return;
        };

        context.execute_background(PluginTask::LogOversamplingMode {
            engaged,
            block_len: u32::try_from(block_len).unwrap_or(u32::MAX),
            chunk_frames: event.chunk_frames,
            latency_before: self.reported_latency,
            latency_after: event.latency_after,
            sample_rate: self.sample_rate,
        });
    }

    /// What to report to the host about this instance's tail.
    ///
    /// Derived from what is actually in the chain right now (see
    /// `Engine::tail_seconds`), not from a constant and not from the IR alone —
    /// presets often run with no cab, and delay/reverb ring without one.
    ///
    /// The floor is the plugin's own reported latency: even a chain that does
    /// not ring at all has in-flight audio sitting in the oversampling FIFO and
    /// the pitch shifter, and reporting `Normal` while that is queued would let
    /// the host suspend before it drains.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn tail_status(&self) -> ProcessStatus {
        let Some(engine) = self.engine.as_ref() else {
            return ProcessStatus::Normal;
        };

        let tail_seconds = engine.tail_seconds();
        if tail_seconds >= KEEP_ALIVE_THRESHOLD_SECONDS {
            return ProcessStatus::KeepAlive;
        }

        let ring = (self.sample_rate * tail_seconds) as u32;
        let in_flight = u32::try_from(engine.latency_frames()).unwrap_or(u32::MAX);
        let tail = ring.max(in_flight);

        if tail == 0 {
            // Nothing rings and nothing is buffered: let the host idle us.
            ProcessStatus::Normal
        } else {
            ProcessStatus::Tail(tail)
        }
    }
}

impl Plugin for RustortionPlugin {
    const NAME: &'static str = "Rustortion";
    const VENDOR: &'static str = "OpenSauce";
    const URL: &'static str = "https://github.com/OpenSauce/rustortion";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        // Stereo
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
        // Mono fallback
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = PluginTask;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let shared = self.shared.clone();

        Box::new(move |task| {
            // Logging tasks carry no engine work, so handle them before the
            // engine-handle lookup (which is `None` once deactivated).
            match task {
                PluginTask::LogRuntimeIssue(msg) => {
                    nih_log!("{msg}");
                    return;
                }
                PluginTask::LogOversamplingMode {
                    engaged,
                    block_len,
                    chunk_frames,
                    latency_before,
                    latency_after,
                    sample_rate,
                } => {
                    log_oversampling_mode(
                        engaged,
                        block_len,
                        chunk_frames,
                        latency_before,
                        latency_after,
                        sample_rate,
                    );
                    return;
                }
                _ => {}
            }

            let handle = shared.engine_handle.lock().ok().and_then(|g| g.clone());
            let Some(handle) = handle else { return };

            match task {
                PluginTask::LogRuntimeIssue(_) | PluginTask::LogOversamplingMode { .. } => {
                    unreachable!("handled above")
                }
                PluginTask::LoadPreset(name) => {
                    let mgr = shared.preset_manager.lock().ok().and_then(|g| g.clone());
                    let loader = shared.ir_loader.lock().ok().and_then(|g| g.clone());
                    let sample_rate = f32::from_bits(shared.sample_rate.load(Ordering::Relaxed));
                    let os_factor = shared.active_oversampling.load(Ordering::Relaxed);
                    do_load_preset(
                        &handle,
                        mgr.as_deref(),
                        loader.as_deref(),
                        sample_rate,
                        os_factor,
                        &name,
                    );
                }
                PluginTask::ChangeOversamplingAndReload {
                    factor,
                    preset_name,
                } => {
                    let sample_rate = f32::from_bits(shared.sample_rate.load(Ordering::Relaxed));
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let sample_rate_usize = sample_rate as usize;
                    let max_buf = shared.max_buffer_size.load(Ordering::Relaxed) as usize;
                    match rustortion_core::audio::samplers::Samplers::new_variable_block(
                        max_buf,
                        f64::from(factor),
                        sample_rate_usize,
                    ) {
                        Ok(samplers) => {
                            handle.set_samplers(samplers);
                            // Update active oversampling AFTER samplers are set,
                            // so effective_sample_rate() stays consistent.
                            shared.active_oversampling.store(factor, Ordering::Relaxed);

                            // Rebuild the amp chain at the new effective rate.
                            // Prefer gui_stages (in-session edits) over reloading
                            // from disk, which would wipe user modifications.
                            #[allow(clippy::cast_precision_loss)]
                            let effective_sr = sample_rate * factor as f32;
                            let stages = shared.take_gui_stages().or_else(|| {
                                shared.preset_manager.lock().ok().and_then(|g| {
                                    g.as_ref().and_then(|mgr| {
                                        preset_name.as_deref().and_then(|name| {
                                            mgr.get_preset_by_name(name).map(|p| p.stages.clone())
                                        })
                                    })
                                })
                            });
                            if let Some(stages) = &stages {
                                let mut chain = rustortion_core::amp::chain::AmplifierChain::new();
                                for cfg in stages {
                                    chain.add_stage(cfg.to_runtime(effective_sr));
                                }
                                for (i, cfg) in stages.iter().enumerate() {
                                    if cfg.bypassed() {
                                        chain.set_bypassed(i, true);
                                    }
                                }
                                handle.set_amp_chain(chain);
                                // Re-store gui_stages since take_gui_stages consumed them
                                shared.store_gui_stages(stages);
                            }
                        }
                        Err(e) => nih_log!("Failed to create samplers: {e}"),
                    }
                }
            }
        })
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(editor::PluginEditor::new(
            self.params.clone(),
            self.shared.clone(),
        )))
    }

    #[allow(clippy::too_many_lines)]
    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.reported.reset();
        // REV-24: the engine built below starts with its own default IR gain,
        // so the `process()` delta check must fire on the very first block to
        // push the real parameter value into it. A sentinel no gain can equal
        // guarantees that; carrying the previous activation's value over is
        // what used to swallow the correction after a reload. (`NEG_INFINITY`,
        // not `NAN` — a `NaN` comparison is false and would swallow it again.)
        self.last_ir_gain = f32::NEG_INFINITY;
        self.sample_rate = buffer_config.sample_rate;
        self.shared
            .sample_rate
            .store(buffer_config.sample_rate.to_bits(), Ordering::Relaxed);
        self.shared
            .max_buffer_size
            .store(buffer_config.max_buffer_size, Ordering::Relaxed);

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sample_rate = buffer_config.sample_rate as usize;
        let max_buffer_size = buffer_config.max_buffer_size as usize;

        let max_ir_samples = sample_rate * 35 / 1000; // 35ms max IR (cab sim only, no room tail)
        let ir_cabinet = rustortion_core::ir::cabinet::IrCabinet::new(
            rustortion_core::ir::cabinet::ConvolverType::Fir,
            max_ir_samples,
        );

        // Read oversampling factor from persisted param, normalized to a valid
        // power-of-two in {1, 2, 4, 8, 16}. Corrupted/legacy values are rounded
        // down to the nearest supported factor.
        let raw_os = self.params.oversampling_factor.load(Ordering::Relaxed);
        let os_factor = match raw_os {
            16.. => 16,
            8..=15 => 8,
            4..=7 => 4,
            2..=3 => 2,
            _ => 1,
        };
        self.active_oversampling = os_factor;
        self.shared
            .requested_oversampling
            .store(os_factor, Ordering::Relaxed);
        self.shared
            .active_oversampling
            .store(os_factor, Ordering::Relaxed);
        let oversample_factor = f64::from(os_factor);

        match Engine::new_for_plugin(
            sample_rate,
            max_buffer_size,
            Some(ir_cabinet),
            oversample_factor,
        ) {
            Ok((engine, handle, rt_drop_rx)) => {
                self.engine = Some(engine);
                self.engine_handle = Some(handle.clone());
                self.rt_drop_thread = Some(std::thread::spawn(move || {
                    rt_drop_rx.run();
                }));

                // Store handle in shared state for background tasks
                if let Ok(mut h) = self.shared.engine_handle.lock() {
                    *h = Some(handle);
                }

                // Initialize IR loader — used for WAV decoding (resampling, normalization).
                // Factory IRs are embedded; this also supports user IRs from ~/.config.
                let ir_dir = dirs::config_dir()
                    .unwrap_or_default()
                    .join("rustortion")
                    .join("impulse_responses");
                match IrLoader::new(&ir_dir, sample_rate) {
                    Ok(loader) => {
                        let loader = Arc::new(loader);
                        if let Ok(mut l) = self.shared.ir_loader.lock() {
                            *l = Some(loader);
                        }
                    }
                    Err(e) => nih_log!("Failed to init IR loader: {e}"),
                }

                // Load user NAM models from ~/.config/rustortion/nam into the
                // process-global registry so the NAM stage can resolve models.
                let nam_dir = user_nam_dir();
                match rustortion_core::nam::NamLoader::new(&nam_dir) {
                    Ok(loader) => {
                        nih_log!("Loaded {} NAM model(s)", loader.available_names().len());
                        rustortion_core::nam::registry::init_from_loader(&loader);
                    }
                    Err(e) => nih_log!("Failed to init NAM loader: {e}"),
                }

                // Load factory presets (embedded in binary)
                let factory_presets = factory::load_factory_presets();
                let names: Vec<String> = factory_presets.iter().map(|p| p.name.clone()).collect();
                self.preset_names.clone_from(&names);
                if let Ok(mut editor_names) = self.editor_preset_names.lock() {
                    editor_names.clone_from(&names);
                }
                let manager = Arc::new(rustortion_core::preset::Manager::new_from_presets(
                    factory_presets,
                ));
                if let Ok(mut m) = self.shared.preset_manager.lock() {
                    *m = Some(manager);
                }

                // Pre-allocate audio buffers
                self.input_buf.resize(max_buffer_size, 0.0);
                self.output_buf.resize(max_buffer_size, 0.0);

                // Re-load chain state: prefer DAW-persisted chain (user may have
                // added/removed stages), fall back to preset from disk.
                let restored_idx = self.params.preset_idx.value();
                self.last_preset_idx = restored_idx;

                // Prefer gui_stages (editor's in-session state) over chain_state
                // (DAW persist, may be stale due to nih-plug re-deserialization).
                let persisted_stages = self
                    .shared
                    .gui_stages
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .or_else(|| self.params.chain_state.lock().ok().and_then(|g| g.clone()));

                if let Some(handle) = &self.engine_handle {
                    if let Some(stages) = &persisted_stages {
                        // Restore from DAW-persisted chain state
                        #[allow(clippy::cast_precision_loss)]
                        let effective_sr = self.sample_rate * os_factor as f32;
                        let mut chain = rustortion_core::amp::chain::AmplifierChain::new();
                        for stage_cfg in stages {
                            chain.add_stage(stage_cfg.to_runtime(effective_sr));
                        }
                        for (i, stage_cfg) in stages.iter().enumerate() {
                            if stage_cfg.bypassed() {
                                chain.set_bypassed(i, true);
                            }
                        }
                        handle.set_amp_chain(chain);

                        // Also load IR/filters/pitch from preset (those are
                        // persisted via nih-plug params and applied separately)
                        #[allow(clippy::cast_sign_loss)]
                        if let Some(name) = self.preset_names.get(restored_idx as usize).cloned()
                            && let Some(mgr) = self
                                .shared
                                .preset_manager
                                .lock()
                                .ok()
                                .and_then(|g| g.clone())
                            && let Some(preset) = mgr.get_preset_by_name(&name)
                        {
                            if let Some(ir_name) = &preset.ir_name {
                                let loader =
                                    self.shared.ir_loader.lock().ok().and_then(|g| g.clone());
                                if let Some(loader) = &loader {
                                    if let Some(bytes) = factory::get_factory_ir(ir_name) {
                                        ir_helper::load_and_set_ir_from_bytes(
                                            handle,
                                            loader,
                                            ir_name,
                                            &bytes,
                                            self.sample_rate,
                                        );
                                    } else {
                                        ir_helper::load_and_set_ir(
                                            handle,
                                            loader,
                                            ir_name,
                                            self.sample_rate,
                                        );
                                    }
                                }
                            }
                            handle.set_ir_gain(preset.ir_gain);
                            handle.set_pitch_shift(preset.pitch_shift_semitones);
                        }
                    } else {
                        // No persisted chain — fall back to loading preset from disk
                        #[allow(clippy::cast_sign_loss)]
                        if let Some(name) = self.preset_names.get(restored_idx as usize).cloned() {
                            let mgr = self
                                .shared
                                .preset_manager
                                .lock()
                                .ok()
                                .and_then(|g| g.clone());
                            let loader = self.shared.ir_loader.lock().ok().and_then(|g| g.clone());
                            do_load_preset(
                                handle,
                                mgr.as_deref(),
                                loader.as_deref(),
                                self.sample_rate,
                                os_factor,
                                &name,
                            );
                        }
                    }

                    // Seed gui_stages from DAW-persisted chain state only if
                    // the editor hasn't already stored its own (newer) data.
                    // nih-plug can re-deserialize chain_state at any time,
                    // reverting our in-memory writes, so gui_stages is the
                    // authoritative in-session source of truth.
                    let gui_already_set = self.shared.gui_stages.lock().is_ok_and(|g| g.is_some());
                    if !gui_already_set && let Some(stages) = persisted_stages {
                        self.shared.store_gui_stages(&stages);
                    }
                }

                // Report plugin delay compensation. The chain and pitch shifter
                // are configured above via engine messages, which the engine
                // only applies on its next `process()` call, so this initial
                // figure covers the oversampling path; `process()` re-reports
                // as soon as the real latency differs.
                let latency = self
                    .engine
                    .as_ref()
                    .map_or(0, |e| u32::try_from(e.latency_frames()).unwrap_or(u32::MAX));
                context.set_latency_samples(latency);
                self.reported_latency = latency;

                true
            }
            Err(e) => {
                nih_log!("Failed to initialize engine: {e}");
                false
            }
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check for preset change from the GUI (preset_idx is a nih-plug param,
        // so it is persisted with DAW project state automatically)
        let idx = self.params.preset_idx.value();
        if idx != self.last_preset_idx {
            #[allow(clippy::cast_sign_loss)]
            if let Some(name) = self.preset_names.get(idx as usize) {
                context.execute_background(PluginTask::LoadPreset(name.clone()));
            }
            self.last_preset_idx = idx;
        }

        // Check for oversampling change (read from SharedState, written by GUI)
        let requested_os = self
            .shared
            .requested_oversampling
            .load(Ordering::Relaxed)
            .clamp(1, 16);
        if requested_os != self.active_oversampling {
            #[allow(clippy::cast_sign_loss)]
            let preset_name = self
                .preset_names
                .get(self.last_preset_idx as usize)
                .cloned();
            context.execute_background(PluginTask::ChangeOversamplingAndReload {
                factor: requested_os,
                preset_name,
            });
            self.active_oversampling = requested_os;
        }

        // Apply IR gain from DAW parameter
        if let Some(handle) = &self.engine_handle {
            #[allow(clippy::cast_possible_truncation)]
            let ir_gain = self
                .params
                .ir_gain
                .smoothed
                .next_step(buffer.samples() as u32);
            if (ir_gain - self.last_ir_gain).abs() > f32::EPSILON {
                handle.set_ir_gain(ir_gain);
                self.last_ir_gain = ir_gain;
            }
        }

        let Some(engine) = &mut self.engine else {
            // No engine: mute rather than fall through. The host copies input
            // into the output buffer before calling us, so returning without
            // writing would pass the unprocessed guitar through at full level.
            silence(buffer, 0);
            return self.tail_status();
        };

        {
            // `input_buf`/`output_buf` are sized once in `initialize()` to the
            // host's declared `max_buffer_size`. A host that exceeds its own
            // declaration would otherwise panic here on the audio thread and
            // take the DAW down with it, so clamp and silence the excess —
            // dropped frames beat an aborted process.
            let num_samples = buffer.samples().min(self.input_buf.len());
            if num_samples < buffer.samples() {
                silence(buffer, num_samples);
                if self.reported.claim(ReportOnce::OVERSIZED_BLOCK) {
                    context.execute_background(PluginTask::LogRuntimeIssue(
                        "Host supplied a block larger than the max_buffer_size it declared; \
                         the excess frames were silenced",
                    ));
                }
            }

            let input_buf = &mut self.input_buf[..num_samples];
            let output_buf = &mut self.output_buf[..num_samples];

            sum_to_mono(buffer, input_buf, num_samples);

            let failed = engine.process(input_buf, output_buf).is_err();
            let nonfinite = engine.take_nonfinite_seen();
            // Plain integer reads; the message is formatted off the RT thread.
            let engaged_block = engine.take_fifo_engaged_event();
            let fast_path_block = engine.take_fast_path_event();
            let chunk_frames = u32::try_from(engine.resampler_chunk_frames()).unwrap_or(u32::MAX);

            if failed {
                // Never let the host's dry input pass through: with no output
                // written the DAW would hear the unprocessed guitar at full
                // level, which on a high-gain amp sim is both wrong and loud.
                for ch in buffer.as_slice().iter_mut() {
                    ch[..num_samples].fill(0.0);
                }
                if self.reported.claim(ReportOnce::PROCESS_ERROR) {
                    context.execute_background(PluginTask::LogRuntimeIssue(
                        "Engine process error; output muted for the affected blocks",
                    ));
                }
                return self.tail_status();
            }

            if nonfinite && self.reported.claim(ReportOnce::NONFINITE) {
                context.execute_background(PluginTask::LogRuntimeIssue(
                    "Non-finite sample(s) reached the master bus and were replaced with silence",
                ));
            }

            let latency_after = u32::try_from(engine.latency_frames()).unwrap_or(u32::MAX);

            // Write mono output to all channels with output level applied
            let output_slices = buffer.as_slice();
            for (i, &sample) in output_buf.iter().enumerate().take(num_samples) {
                let gain = self.params.output_level.smoothed.next();
                for ch in output_slices.iter_mut() {
                    ch[i] = sample * gain;
                }
            }

            // Log the oversampling path *before* re-reporting latency, so the
            // message can quote the previous figure as `latency_before`.
            self.report_oversampling_mode(
                context,
                &OversamplingEvent {
                    engaged_block,
                    fast_path_block,
                    chunk_frames,
                    latency_after,
                },
            );

            // Latency changes when the user changes oversampling or pitch
            // shift, or when the FIFO engages, so re-report whenever it moves.
            if latency_after != self.reported_latency {
                context.set_latency_samples(latency_after);
                self.reported_latency = latency_after;
            }
        }

        // Reporting `Normal` unconditionally lets the host suspend us the
        // moment the input goes quiet, chopping the IR, delay, and reverb
        // tails on transport stop. `tail_status` reports `Normal` only when
        // there is genuinely nothing left to hear.
        self.tail_status()
    }

    /// Flush every delay line, filter, buffer and tail in the engine.
    ///
    /// nih-plug calls this after each `initialize()` and "at any other time
    /// from the audio thread" — in practice, whenever the host relocates the
    /// transport. `Engine::reset` is written to that contract: it zeroes state
    /// in place and never allocates, locks, or does I/O, so this must not be
    /// rerouted through the engine-rebuild path.
    fn reset(&mut self) {
        if let Some(engine) = self.engine.as_mut() {
            engine.reset();
        }
    }

    fn deactivate(&mut self) {
        // 1. Clear shared state first -- background tasks become no-ops
        if let Ok(mut h) = self.shared.engine_handle.lock() {
            *h = None;
        }
        if let Ok(mut l) = self.shared.ir_loader.lock() {
            *l = None;
        }
        if let Ok(mut m) = self.shared.preset_manager.lock() {
            *m = None;
        }
        // 2. Drop engine resources
        self.engine = None;
        self.engine_handle = None;
        // 3. Join rt_drop_thread
        if let Some(thread) = self.rt_drop_thread.take() {
            let _ = thread.join();
        }
    }
}

impl ClapPlugin for RustortionPlugin {
    const CLAP_ID: &'static str = "com.opensauce.rustortion";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Guitar/bass amp simulator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for RustortionPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"RustortionPlugV1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(RustortionPlugin);
nih_export_vst3!(RustortionPlugin);
