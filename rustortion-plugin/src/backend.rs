use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use rustortion_core::amp::chain::AmplifierChain;
use rustortion_core::amp::stages::Stage;
use rustortion_core::amp::stages::filter::{FilterStage, FilterType};
use rustortion_core::audio::engine::EngineHandle;
use rustortion_core::ir::loader::IrLoader;
use rustortion_core::preset::InputFilterConfig;
use rustortion_core::preset::stage_config::StageConfig;
use rustortion_ui::backend::{Capabilities, ExternalEvent, ParamBackend};

use crate::params::RustortionParams;

pub struct PluginBackend {
    engine_handle: EngineHandle,
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    ir_loader: Option<Arc<IrLoader>>,
    capabilities: Capabilities,
    sample_rate: f32,
    oversampling_factor: u32,
}

impl PluginBackend {
    pub fn new(
        engine_handle: EngineHandle,
        params: Arc<RustortionParams>,
        context: Arc<dyn GuiContext>,
        ir_loader: Option<Arc<IrLoader>>,
        sample_rate: f32,
        oversampling_factor: u32,
    ) -> Self {
        Self {
            engine_handle,
            params,
            context,
            ir_loader,
            capabilities: Capabilities::plugin(),
            sample_rate,
            oversampling_factor,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn effective_sample_rate(&self) -> f32 {
        self.sample_rate * self.oversampling_factor as f32
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
        match loader.load_by_name(name) {
            Ok(ir_samples) => {
                // Truncate IR to 35ms for cab sim (no room reverb tail)
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let max_ir_len = (self.sample_rate * 35.0 / 1000.0) as usize;
                let truncated_len = ir_samples.len().min(max_ir_len);
                let mut convolver =
                    rustortion_core::ir::convolver::Convolver::new_fir(truncated_len);
                if let Err(e) = convolver.set_ir(&ir_samples[..truncated_len]) {
                    log::error!("Failed to set IR: {e}");
                } else {
                    self.engine_handle.swap_ir_convolver(
                        rustortion_core::audio::engine::PreparedIr {
                            name: name.to_string(),
                            convolver,
                        },
                    );
                }
            }
            Err(e) => log::error!("Failed to load IR '{name}': {e}"),
        }
    }

    fn set_ir_bypass(&self, bypassed: bool) {
        self.engine_handle.set_ir_bypass(bypassed);
    }

    fn set_ir_gain(&self, gain: f32) {
        self.engine_handle.set_ir_gain(gain);
        // Also update the nih-plug parameter for DAW automation
        let _ = &self.params;
        let _ = &self.context;
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
    }

    fn set_pitch_shift(&self, semitones: i32) {
        self.engine_handle.set_pitch_shift(semitones);
    }

    fn set_oversampling(&self, _factor: u32) {
        // Oversampling changes in plugin mode are handled through the
        // nih-plug parameter system and the process() loop. This trait
        // method is a no-op for the plugin backend.
    }

    fn sample_rate(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sr = self.sample_rate as u32;
        sr
    }

    fn oversampling_factor(&self) -> u32 {
        self.oversampling_factor
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn get_available_irs(&self) -> Vec<String> {
        self.ir_loader
            .as_ref()
            .map_or_else(Vec::new, |loader| loader.available_ir_names())
    }

    fn get_peak_meter_info(&self) -> Option<ExternalEvent> {
        // Plugin mode does not poll peak meters from the GUI; the DAW
        // provides its own metering.
        None
    }
}
