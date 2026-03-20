use nih_plug::prelude::*;
use nih_plug_iced::IcedState;
use rustortion_core::audio::engine::{Engine, EngineHandle};
use rustortion_core::ir::loader::IrLoader;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

mod editor;

enum PluginTask {
    LoadPreset(String),
    ChangeOversampling(u8),
}

struct SharedState {
    engine_handle: Mutex<Option<EngineHandle>>,
    ir_loader: Mutex<Option<Arc<IrLoader>>>,
    preset_manager: Mutex<Option<Arc<rustortion_core::preset::Manager>>>,
    sample_rate: AtomicU32,
    max_buffer_size: AtomicU32,
    oversampling_idx: AtomicU8,
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
    current_preset_idx: Arc<AtomicUsize>,
    last_preset_idx: usize,
    last_ir_gain: f32,
    active_oversampling: u8,
    input_buf: Vec<f32>,
    output_buf: Vec<f32>,
}

#[derive(Params)]
struct RustortionParams {
    #[persist = "editor-state"]
    editor_state: Arc<IcedState>,

    #[id = "output_level"]
    output_level: FloatParam,

    #[id = "ir_gain"]
    ir_gain: FloatParam,

    #[persist = "oversampling"]
    oversampling_idx: Arc<AtomicU8>,
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
            }),
            preset_names: Vec::new(),
            editor_preset_names: Arc::new(Mutex::new(Vec::new())),
            current_preset_idx: Arc::new(AtomicUsize::new(0)),
            last_preset_idx: 0,
            last_ir_gain: util::db_to_gain(-20.0),
            active_oversampling: 1, // 1 = 2x
            input_buf: Vec::new(),
            output_buf: Vec::new(),
        }
    }
}

impl Default for RustortionParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            output_level: FloatParam::new(
                "Output Level",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(12.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 12.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            ir_gain: FloatParam::new(
                "Cabinet Level",
                util::db_to_gain(-20.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            oversampling_idx: Arc::new(AtomicU8::new(1)), // 1 = 2x oversampling
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
            match loader.load_by_name(ir_name) {
                Ok(ir_samples) => {
                    // Truncate IR to 35ms for cab sim (no room reverb tail)
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let max_ir_len = (sample_rate * 35.0 / 1000.0) as usize;
                    let truncated_len = ir_samples.len().min(max_ir_len);
                    let mut convolver =
                        rustortion_core::ir::convolver::Convolver::new_fir(truncated_len);
                    if let Err(e) = convolver.set_ir(&ir_samples[..truncated_len]) {
                        nih_log!("Failed to set IR: {e}");
                    } else {
                        handle.swap_ir_convolver(rustortion_core::audio::engine::PreparedIr {
                            name: ir_name.clone(),
                            convolver,
                        });
                    }
                }
                Err(e) => nih_log!("Failed to load IR '{ir_name}': {e}"),
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
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.editor_preset_names.clone(),
            self.current_preset_idx.clone(),
            self.params.oversampling_idx.clone(),
        )
    }

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

                // Initialize IR loader
                let ir_dir = find_ir_directory();
                match IrLoader::new(&ir_dir, sample_rate) {
                    Ok(loader) => {
                        let loader = Arc::new(loader);
                        if let Ok(mut l) = self.shared.ir_loader.lock() {
                            *l = Some(loader);
                        }
                    }
                    Err(e) => nih_log!("Failed to init IR loader: {e}"),
                }

                // Load presets
                let preset_dir = dirs::config_dir().map_or_else(
                    || {
                        nih_log!("Could not determine config directory; preset loading may fail");
                        std::path::PathBuf::from("rustortion").join("presets")
                    },
                    |dir| dir.join("rustortion").join("presets"),
                );

                match rustortion_core::preset::Manager::new(&preset_dir) {
                    Ok(manager) => {
                        let names: Vec<String> = manager
                            .get_presets()
                            .iter()
                            .map(|p| p.name.clone())
                            .collect();
                        self.preset_names.clone_from(&names);
                        if let Ok(mut editor_names) = self.editor_preset_names.lock() {
                            editor_names.clone_from(&names);
                        }
                        let manager = Arc::new(manager);
                        if let Ok(mut m) = self.shared.preset_manager.lock() {
                            *m = Some(manager);
                        }
                    }
                    Err(e) => nih_log!("Failed to load presets: {e}"),
                }

                // Pre-allocate audio buffers
                self.input_buf.resize(max_buffer_size, 0.0);
                self.output_buf.resize(max_buffer_size, 0.0);

                // Re-load active preset (handles reactivation)
                if let Some(name) = self.preset_names.get(self.last_preset_idx).cloned()
                    && let Some(handle) = &self.engine_handle
                {
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
        // Check for preset change from the GUI
        let idx = self.current_preset_idx.load(Ordering::Relaxed);
        if idx != self.last_preset_idx
            && let Some(name) = self.preset_names.get(idx)
        {
            context.execute_background(PluginTask::LoadPreset(name.clone()));
            self.last_preset_idx = idx;
        }

        // Check for oversampling change
        let os_idx = self.params.oversampling_idx.load(Ordering::Relaxed).min(4);
        if os_idx != self.active_oversampling {
            context.execute_background(PluginTask::ChangeOversampling(os_idx));
            self.active_oversampling = os_idx;
            // Reload preset so time-based stages get the correct effective sample rate
            if let Some(name) = self.preset_names.get(self.last_preset_idx) {
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

fn find_ir_directory() -> std::path::PathBuf {
    // Check env var
    if let Ok(path) = std::env::var("RUSTORTION_IR_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    // Check ~/.config/rustortion/impulse_responses/
    if let Some(config) = dirs::config_dir() {
        let p = config.join("rustortion").join("impulse_responses");
        if p.exists() {
            return p;
        }
    }
    // Check next to executable
    if let Ok(exe) = std::env::current_exe() {
        let bundled = exe.parent().map(|p| p.join("impulse_responses"));
        if bundled.as_ref().is_some_and(|p| p.exists()) {
            return bundled.unwrap();
        }
    }
    // Fallback
    dirs::config_dir()
        .unwrap_or_default()
        .join("rustortion")
        .join("impulse_responses")
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
