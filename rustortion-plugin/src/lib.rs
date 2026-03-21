use nih_plug::prelude::*;
use rustortion_core::audio::engine::{Engine, EngineHandle};
use rustortion_core::ir::loader::IrLoader;
use rustortion_core::preset::stage_config::StageConfig;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

mod backend;
mod editor;
mod factory;
mod ir_helper;
pub mod params;

use params::RustortionParams;

enum PluginTask {
    LoadPreset(String),
    ChangeOversampling(u8),
}

pub(crate) struct SharedState {
    engine_handle: Mutex<Option<EngineHandle>>,
    ir_loader: Mutex<Option<Arc<IrLoader>>>,
    preset_manager: Mutex<Option<Arc<rustortion_core::preset::Manager>>>,
    sample_rate: AtomicU32,
    max_buffer_size: AtomicU32,
    oversampling_idx: AtomicU8,
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
    active_oversampling: u8,
    input_buf: Vec<f32>,
    output_buf: Vec<f32>,
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
                oversampling_idx: AtomicU8::new(1),
                gui_stages: Mutex::new(None),
            }),
            preset_names: Vec::new(),
            editor_preset_names: Arc::new(Mutex::new(Vec::new())),
            last_preset_idx: -1,
            last_ir_gain: util::db_to_gain(-20.0),
            active_oversampling: 1, // 1 = 2x
            input_buf: Vec::new(),
            output_buf: Vec::new(),
        }
    }
}

fn do_load_preset(
    handle: &EngineHandle,
    manager: Option<&rustortion_core::preset::Manager>,
    ir_loader: Option<&IrLoader>,
    sample_rate: f32,
    oversampling_idx: u8,
    name: &str,
) {
    let Some(manager) = manager else {
        return;
    };
    let Some(preset) = manager.get_preset_by_name(name) else {
        return;
    };

    // Stages in the amp chain run at the oversampled rate
    let effective_sr = sample_rate * 2.0_f32.powi(i32::from(oversampling_idx));

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
            let handle = shared.engine_handle.lock().ok().and_then(|g| g.clone());
            let Some(handle) = handle else { return };

            match task {
                PluginTask::LoadPreset(name) => {
                    let mgr = shared.preset_manager.lock().ok().and_then(|g| g.clone());
                    let loader = shared.ir_loader.lock().ok().and_then(|g| g.clone());
                    let sample_rate = f32::from_bits(shared.sample_rate.load(Ordering::Relaxed));
                    let os_idx = shared.oversampling_idx.load(Ordering::Relaxed);
                    do_load_preset(
                        &handle,
                        mgr.as_deref(),
                        loader.as_deref(),
                        sample_rate,
                        os_idx,
                        &name,
                    );
                }
                PluginTask::ChangeOversampling(idx) => {
                    let sample_rate = f32::from_bits(shared.sample_rate.load(Ordering::Relaxed));
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let sample_rate_usize = sample_rate as usize;
                    let max_buf = shared.max_buffer_size.load(Ordering::Relaxed) as usize;
                    let factor = 2.0_f64.powi(i32::from(idx));
                    match rustortion_core::audio::samplers::Samplers::new(
                        max_buf,
                        factor,
                        sample_rate_usize,
                    ) {
                        Ok(samplers) => handle.set_samplers(samplers),
                        Err(e) => nih_log!("Failed to create samplers: {e}"),
                    }
                    shared.oversampling_idx.store(idx, Ordering::Relaxed);
                }
            }
        })
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // Don't open the editor if the engine isn't initialized yet
        if self.shared.engine_handle.lock().ok()?.is_none() {
            return None;
        }
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
        _context: &mut impl InitContext<Self>,
    ) -> bool {
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

        // Read oversampling from persisted param (clamped to valid range)
        let os_idx = self.params.oversampling_idx.load(Ordering::Relaxed).min(4);
        self.active_oversampling = os_idx;
        self.shared
            .oversampling_idx
            .store(os_idx, Ordering::Relaxed);
        let oversample_factor = 2.0_f64.powi(i32::from(os_idx));

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
                        let effective_sr = self.sample_rate * 2.0_f32.powi(i32::from(os_idx));
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
                                os_idx,
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

        // Check for oversampling change
        let os_idx = self.params.oversampling_idx.load(Ordering::Relaxed).min(4);
        if os_idx != self.active_oversampling {
            context.execute_background(PluginTask::ChangeOversampling(os_idx));
            self.active_oversampling = os_idx;
            // Reload preset so time-based stages get the correct effective sample rate
            #[allow(clippy::cast_sign_loss)]
            if let Some(name) = self.preset_names.get(self.last_preset_idx as usize) {
                context.execute_background(PluginTask::LoadPreset(name.clone()));
            }
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

        if let Some(engine) = &mut self.engine {
            let num_samples = buffer.samples();
            let input_buf = &mut self.input_buf[..num_samples];
            let output_buf = &mut self.output_buf[..num_samples];

            // Sum all input channels to mono
            {
                let channel_slices = buffer.as_slice_immutable();
                if !channel_slices.is_empty() {
                    #[allow(clippy::cast_precision_loss)] // channel count < 2^24
                    let scale = 1.0 / channel_slices.len() as f32;
                    for i in 0..num_samples {
                        let mut sum = 0.0;
                        for ch in channel_slices {
                            sum += ch[i];
                        }
                        input_buf[i] = sum * scale;
                    }
                }
            }

            if let Err(e) = engine.process(input_buf, output_buf) {
                nih_log!("Engine process error: {e}");
                return ProcessStatus::Normal;
            }

            // Write mono output to all channels with output level applied
            let output_slices = buffer.as_slice();
            for i in 0..num_samples {
                let gain = self.params.output_level.smoothed.next();
                for ch in output_slices.iter_mut() {
                    ch[i] = output_buf[i] * gain;
                }
            }
        }

        ProcessStatus::Normal
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
