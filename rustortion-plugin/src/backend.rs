use std::sync::Arc;
use std::sync::atomic::Ordering;

use nih_plug::prelude::{GuiContext, Param};
use rustortion_core::amp::chain::AmplifierChain;
use rustortion_core::amp::stages::Stage;
use rustortion_core::amp::stages::filter::{FilterStage, FilterType};
use rustortion_core::audio::engine::EngineHandle;
use rustortion_core::ir::loader::IrLoader;
use rustortion_core::preset::InputFilterConfig;
use rustortion_core::preset::stage_config::StageConfig;
use rustortion_ui::backend::{Capabilities, ExternalEvent, ParamBackend};

use crate::SharedState;
use crate::params::RustortionParams;

/// GUI-side driver for the plugin's engine.
///
/// Holds **no** snapshot of engine state. Everything the engine owns — the
/// message channel, the IR loader, the sample rate — is resolved from
/// [`SharedState`] on each call.
///
/// That is not a style preference. `deactivate()` drops the engine (and with
/// it the receiving end of the channel) and nulls the shared slots; a
/// subsequent `initialize()` rebuilds all of them, possibly at a different
/// sample rate. nih-plug also permits `initialize()` to run repeatedly with no
/// intervening `deactivate()` while restoring state. Anything captured when the
/// editor window opened outlives every one of those transitions, and the editor
/// window is not reopened in between — so a captured value is a value that goes
/// quietly wrong while the UI keeps looking alive.
pub struct PluginBackend {
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    shared_state: Arc<SharedState>,
    capabilities: Capabilities,
}

impl PluginBackend {
    pub fn new(
        params: Arc<RustortionParams>,
        context: Arc<dyn GuiContext>,
        shared_state: Arc<SharedState>,
    ) -> Self {
        Self {
            params,
            context,
            shared_state,
            capabilities: Capabilities::plugin(),
        }
    }

    /// The handle for the engine that is running *right now*, or `None` between
    /// `deactivate()` and the next `initialize()`.
    fn engine(&self) -> Option<EngineHandle> {
        self.shared_state.engine_handle.lock().ok()?.clone()
    }

    /// The IR loader belonging to the current engine.
    ///
    /// Rebuilt by `initialize()` bound to the then-current sample rate, since
    /// the loader is what resamples IRs on the way in. A loader from a previous
    /// activation resamples to the wrong rate; one captured while the plugin
    /// was inactive is `None` forever and silently disables IR selection.
    fn ir_loader(&self) -> Option<Arc<IrLoader>> {
        self.shared_state.ir_loader.lock().ok()?.clone()
    }

    /// The host's sample rate, or `None` before the first `initialize()`.
    ///
    /// The shared slot starts at zero, and a host may open the editor before it
    /// activates the plugin. Building a stage at 0 Hz is not merely inaccurate:
    /// `FilterStage`'s alpha divides by the rate, so a lowpass comes out with a
    /// NaN coefficient. Callers bail rather than hand that to a live engine.
    fn host_sample_rate(&self) -> Option<f32> {
        let sr = f32::from_bits(self.shared_state.sample_rate.load(Ordering::Relaxed));
        (sr.is_finite() && sr > 0.0).then_some(sr)
    }

    /// Engine handle plus the effective rate to build stages at.
    ///
    /// Both or neither: a stage built at the wrong rate is no better than one
    /// that never arrives, and it is harder to notice.
    fn engine_with_effective_rate(&self) -> Option<(EngineHandle, f32)> {
        Some((self.engine()?, self.effective_sample_rate()?))
    }

    /// Note a GUI edit that could not be delivered.
    ///
    /// No engine is a legitimate state, not an error, so this is `debug` rather
    /// than `warn`. But it is also how the stale-handle bug stayed hidden for
    /// four months: every dropped edit went into a log line nobody was running
    /// with. Leave a trail for the next `NIH_LOG=debug` session.
    fn note_dropped_edit(what: &str) {
        log::debug!("GUI edit '{what}' dropped: no active engine");
    }

    /// Read DAW-persisted chain state (from `#[persist]` field).
    pub fn persisted_chain_state(&self) -> Option<Vec<StageConfig>> {
        self.params.chain_state.lock().ok()?.clone()
    }

    /// Effective sample rate using the *active* (applied) oversampling factor,
    /// not the requested one. This ensures chain rebuilds match the current
    /// sampler state.
    #[allow(clippy::cast_precision_loss)]
    fn effective_sample_rate(&self) -> Option<f32> {
        let active = self
            .shared_state
            .active_oversampling
            .load(Ordering::Relaxed);
        Some(self.host_sample_rate()? * active as f32)
    }

    /// Notify the host that a parameter value changed from the GUI.
    /// SAFETY: `ptr` must be a valid `ParamPtr` from one of our `RustortionParams` fields.
    fn notify_host_param_changed(&self, ptr: nih_plug::prelude::ParamPtr, normalized: f32) {
        // SAFETY: The ParamPtr comes from our own params struct and is valid for the
        // lifetime of the plugin. The raw_* methods require that the pointer is valid.
        unsafe {
            self.context.raw_begin_set_parameter(ptr);
            self.context.raw_set_parameter_normalized(ptr, normalized);
            self.context.raw_end_set_parameter(ptr);
        }
    }
}

impl ParamBackend for PluginBackend {
    fn set_parameter(&self, stage_idx: usize, name: &'static str, value: f32) {
        let Some(engine) = self.engine() else {
            Self::note_dropped_edit("set_parameter");
            return;
        };
        engine.set_parameter(stage_idx, name, value);
    }

    fn rebuild_stage(&self, stage_idx: usize, config: &StageConfig) {
        // Resolve before building: with no engine the stage would be allocated
        // only to be dropped, and with no rate it would be built wrong.
        let Some((engine, sr)) = self.engine_with_effective_rate() else {
            Self::note_dropped_edit("rebuild_stage");
            return;
        };
        engine.replace_stage(stage_idx, config.to_runtime(sr));
    }

    fn set_amp_chain(&self, stages: &[StageConfig]) {
        let Some((engine, sr)) = self.engine_with_effective_rate() else {
            Self::note_dropped_edit("set_amp_chain");
            return;
        };
        let mut chain = AmplifierChain::new();
        for cfg in stages {
            chain.add_stage(cfg.to_runtime(sr));
        }
        for (i, cfg) in stages.iter().enumerate() {
            if cfg.bypassed() {
                chain.set_bypassed(i, true);
            }
        }
        engine.set_amp_chain(chain);
    }

    fn set_bypass(&self, stage_idx: usize, bypassed: bool) {
        let Some(engine) = self.engine() else {
            Self::note_dropped_edit("set_bypass");
            return;
        };
        engine.set_stage_bypassed(stage_idx, bypassed);
    }

    fn add_stage(&self, idx: usize, config: &StageConfig) {
        let Some((engine, sr)) = self.engine_with_effective_rate() else {
            Self::note_dropped_edit("add_stage");
            return;
        };
        engine.add_stage(idx, config.to_runtime(sr));
    }

    fn remove_stage(&self, idx: usize) {
        let Some(engine) = self.engine() else {
            Self::note_dropped_edit("remove_stage");
            return;
        };
        engine.remove_stage(idx);
    }

    fn swap_stages(&self, a: usize, b: usize) {
        let Some(engine) = self.engine() else {
            Self::note_dropped_edit("swap_stages");
            return;
        };
        engine.swap_stages(a, b);
    }

    fn set_ir(&self, name: &str) {
        // The IR runs after downsampling, so it is built at the host rate, not
        // the oversampled one.
        let (Some(loader), Some(engine), Some(sr)) =
            (self.ir_loader(), self.engine(), self.host_sample_rate())
        else {
            Self::note_dropped_edit("set_ir");
            return;
        };
        // Try embedded factory IR first
        if let Some(bytes) = crate::factory::get_factory_ir(name) {
            crate::ir_helper::load_and_set_ir_from_bytes(&engine, &loader, name, &bytes, sr);
        } else {
            // Fall back to filesystem (user-added IRs)
            crate::ir_helper::load_and_set_ir(&engine, &loader, name, sr);
        }
    }

    fn set_ir_bypass(&self, bypassed: bool) {
        if let Some(engine) = self.engine() {
            engine.set_ir_bypass(bypassed);
        } else {
            Self::note_dropped_edit("set_ir_bypass");
        }
        let param = &self.params.ir_bypass;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(bypassed));
    }

    fn set_ir_gain(&self, gain: f32) {
        if let Some(engine) = self.engine() {
            engine.set_ir_gain(gain);
        } else {
            Self::note_dropped_edit("set_ir_gain");
        }
        let param = &self.params.ir_gain;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(gain));
    }

    fn set_input_filter(&self, filter: &InputFilterConfig) {
        // Input filters sit ahead of the upsampler, so they too are built at the
        // host rate. Host param sync below happens either way — those are the
        // host's parameters, and they stay valid with no engine attached.
        if let (Some(engine), Some(sr)) = (self.engine(), self.host_sample_rate()) {
            let hp: Option<Box<dyn Stage>> = filter.hp_enabled.then(|| {
                Box::new(FilterStage::new(FilterType::Highpass, filter.hp_cutoff, sr))
                    as Box<dyn Stage>
            });
            let lp: Option<Box<dyn Stage>> = filter.lp_enabled.then(|| {
                Box::new(FilterStage::new(FilterType::Lowpass, filter.lp_cutoff, sr))
                    as Box<dyn Stage>
            });
            engine.set_input_filters(hp, lp);
        } else {
            Self::note_dropped_edit("set_input_filter");
        }

        // Sync filter params to host
        let p = &self.params.hp_enabled;
        self.notify_host_param_changed(p.as_ptr(), p.preview_normalized(filter.hp_enabled));
        let p = &self.params.hp_cutoff;
        self.notify_host_param_changed(p.as_ptr(), p.preview_normalized(filter.hp_cutoff));
        let p = &self.params.lp_enabled;
        self.notify_host_param_changed(p.as_ptr(), p.preview_normalized(filter.lp_enabled));
        let p = &self.params.lp_cutoff;
        self.notify_host_param_changed(p.as_ptr(), p.preview_normalized(filter.lp_cutoff));
    }

    fn set_pitch_shift(&self, semitones: i32) {
        if let Some(engine) = self.engine() {
            engine.set_pitch_shift(semitones);
        } else {
            Self::note_dropped_edit("set_pitch_shift");
        }
        let param = &self.params.pitch_shift;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(semitones));
    }

    fn set_preset_index(&self, index: usize) {
        let param = &self.params.preset_idx;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let idx = index as i32;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(idx));
    }

    fn set_oversampling(&self, factor: u32) {
        debug_assert!(
            factor.is_power_of_two() && factor > 0 && factor <= 16,
            "oversampling factor must be a power of two in [1, 16], got {factor}"
        );
        self.shared_state
            .requested_oversampling
            .store(factor, Ordering::Relaxed);
        // Sync to params for DAW project persistence
        self.params
            .oversampling_factor
            .store(factor, Ordering::Relaxed);
        // Mark DAW session dirty so the new value is saved with the project.
        // #[persist] fields are serialized passively and don't trigger a save.
        let param = &self.params.preset_idx;
        let current = param.modulated_normalized_value();
        self.notify_host_param_changed(param.as_ptr(), current);
    }

    /// Zero until the host activates the plugin. Read live so the NAM stage's
    /// rate-mismatch badge reflects the engine that is actually running.
    fn sample_rate(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sr = self.host_sample_rate().unwrap_or(0.0) as u32;
        sr
    }

    fn oversampling_factor(&self) -> u32 {
        self.shared_state
            .requested_oversampling
            .load(Ordering::Relaxed)
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn get_available_irs(&self) -> Vec<String> {
        let mut names = crate::factory::factory_ir_names();
        // Also include any user IRs from filesystem
        if let Some(loader) = self.ir_loader() {
            for name in loader.available_ir_names() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }

    fn get_peak_meter_info(&self) -> Option<ExternalEvent> {
        // Plugin mode does not poll peak meters from the GUI; the DAW
        // provides its own metering.
        None
    }

    fn nam_models_dir(&self) -> Option<std::path::PathBuf> {
        Some(crate::user_nam_dir())
    }

    fn rescan_nam_models(&self) -> Result<usize, String> {
        let dir = crate::user_nam_dir();
        let loader = rustortion_core::nam::NamLoader::new(&dir).map_err(|e| e.to_string())?;
        rustortion_core::nam::registry::init_from_loader(&loader);
        Ok(loader.available_names().len())
    }

    fn persist_chain_state(&self, stages: &[StageConfig]) {
        // Store in SharedState for editor close/reopen within same session
        self.shared_state.store_gui_stages(stages);
        // Store in nih-plug persist field for DAW project save/restore
        if let Ok(mut cs) = self.params.chain_state.lock() {
            *cs = Some(stages.to_vec());
        }
        // Touch preset_idx with its current value to notify the host that
        // state changed. #[persist] fields are serialized passively and
        // don't mark the session dirty on their own.
        let param = &self.params.preset_idx;
        let current = param.modulated_normalized_value();
        self.notify_host_param_changed(param.as_ptr(), current);
    }
}
