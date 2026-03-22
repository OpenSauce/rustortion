use std::sync::Arc;

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
pub struct PluginBackend {
    engine_handle: EngineHandle,
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    ir_loader: Option<Arc<IrLoader>>,
    shared_state: Arc<SharedState>,
    capabilities: Capabilities,
    sample_rate: f32,
}

impl PluginBackend {
    pub fn new(
        engine_handle: EngineHandle,
        params: Arc<RustortionParams>,
        context: Arc<dyn GuiContext>,
        ir_loader: Option<Arc<IrLoader>>,
        shared_state: Arc<SharedState>,
        sample_rate: f32,
    ) -> Self {
        Self {
            engine_handle,
            params,
            context,
            ir_loader,
            shared_state,
            capabilities: Capabilities::plugin(),
            sample_rate,
        }
    }

    /// Read DAW-persisted chain state (from `#[persist]` field).
    pub fn persisted_chain_state(&self) -> Option<Vec<StageConfig>> {
        self.params.chain_state.lock().ok()?.clone()
    }

    /// Effective sample rate using the *active* (applied) oversampling factor,
    /// not the requested one. This ensures chain rebuilds match the current
    /// sampler state.
    #[allow(clippy::cast_precision_loss)]
    fn effective_sample_rate(&self) -> f32 {
        let active = self
            .shared_state
            .active_oversampling
            .load(std::sync::atomic::Ordering::Relaxed);
        self.sample_rate * active as f32
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
        self.engine_handle.set_parameter(stage_idx, name, value);
    }

    fn rebuild_stage(&self, stage_idx: usize, config: &StageConfig) {
        let sr = self.effective_sample_rate();
        let runtime_stage = config.to_runtime(sr);
        self.engine_handle.replace_stage(stage_idx, runtime_stage);
    }

    fn set_amp_chain(&self, stages: &[StageConfig]) {
        let sr = self.effective_sample_rate();
        let mut chain = AmplifierChain::new();
        for cfg in stages {
            chain.add_stage(cfg.to_runtime(sr));
        }
        for (i, cfg) in stages.iter().enumerate() {
            if cfg.bypassed() {
                chain.set_bypassed(i, true);
            }
        }
        self.engine_handle.set_amp_chain(chain);
    }

    fn set_bypass(&self, stage_idx: usize, bypassed: bool) {
        self.engine_handle.set_stage_bypassed(stage_idx, bypassed);
    }

    fn add_stage(&self, idx: usize, config: &StageConfig) {
        let sr = self.effective_sample_rate();
        let runtime_stage = config.to_runtime(sr);
        self.engine_handle.add_stage(idx, runtime_stage);
    }

    fn remove_stage(&self, idx: usize) {
        self.engine_handle.remove_stage(idx);
    }

    fn swap_stages(&self, a: usize, b: usize) {
        self.engine_handle.swap_stages(a, b);
    }

    fn set_ir(&self, name: &str) {
        let Some(loader) = &self.ir_loader else {
            return;
        };
        // Try embedded factory IR first
        if let Some(bytes) = crate::factory::get_factory_ir(name) {
            crate::ir_helper::load_and_set_ir_from_bytes(
                &self.engine_handle,
                loader,
                name,
                &bytes,
                self.sample_rate,
            );
        } else {
            // Fall back to filesystem (user-added IRs)
            crate::ir_helper::load_and_set_ir(&self.engine_handle, loader, name, self.sample_rate);
        }
    }

    fn set_ir_bypass(&self, bypassed: bool) {
        self.engine_handle.set_ir_bypass(bypassed);
        let param = &self.params.ir_bypass;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(bypassed));
    }

    fn set_ir_gain(&self, gain: f32) {
        self.engine_handle.set_ir_gain(gain);
        let param = &self.params.ir_gain;
        self.notify_host_param_changed(param.as_ptr(), param.preview_normalized(gain));
    }

    fn set_input_filter(&self, filter: &InputFilterConfig) {
        let hp: Option<Box<dyn Stage>> = if filter.hp_enabled {
            Some(Box::new(FilterStage::new(
                FilterType::Highpass,
                filter.hp_cutoff,
                self.sample_rate,
            )))
        } else {
            None
        };
        let lp: Option<Box<dyn Stage>> = if filter.lp_enabled {
            Some(Box::new(FilterStage::new(
                FilterType::Lowpass,
                filter.lp_cutoff,
                self.sample_rate,
            )))
        } else {
            None
        };
        self.engine_handle.set_input_filters(hp, lp);

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
        self.engine_handle.set_pitch_shift(semitones);
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
            .store(factor, std::sync::atomic::Ordering::Relaxed);
        // Sync to params for DAW project persistence
        self.params
            .oversampling_factor
            .store(factor, std::sync::atomic::Ordering::Relaxed);
        // Mark DAW session dirty so the new value is saved with the project.
        // #[persist] fields are serialized passively and don't trigger a save.
        let param = &self.params.preset_idx;
        let current = param.modulated_normalized_value();
        self.notify_host_param_changed(param.as_ptr(), current);
    }

    fn sample_rate(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sr = self.sample_rate as u32;
        sr
    }

    fn oversampling_factor(&self) -> u32 {
        self.shared_state
            .requested_oversampling
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn get_available_irs(&self) -> Vec<String> {
        let mut names = crate::factory::factory_ir_names();
        // Also include any user IRs from filesystem
        if let Some(loader) = &self.ir_loader {
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
