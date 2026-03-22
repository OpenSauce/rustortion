use std::sync::atomic::{AtomicU32, Ordering};

use log::error;
use rustortion_core::amp::chain::AmplifierChain;
use rustortion_core::amp::stages::Stage;
use rustortion_core::amp::stages::filter::{FilterStage, FilterType};
use rustortion_core::audio::samplers::Samplers;
use rustortion_core::preset::InputFilterConfig;
use rustortion_core::preset::stage_config::StageConfig;
use rustortion_ui::backend::{Capabilities, ExternalEvent, ParamBackend};

use crate::audio::manager::Manager;

pub struct StandaloneBackend {
    manager: Manager,
    capabilities: Capabilities,
    oversampling_factor: AtomicU32,
}

impl StandaloneBackend {
    pub const fn new(manager: Manager) -> Self {
        let factor = manager.current_oversampling_factor();
        Self {
            manager,
            capabilities: Capabilities::standalone(),
            oversampling_factor: AtomicU32::new(factor),
        }
    }

    pub const fn manager(&self) -> &Manager {
        &self.manager
    }

    pub const fn manager_mut(&mut self) -> &mut Manager {
        &mut self.manager
    }

    fn effective_sample_rate(&self) -> usize {
        self.manager.sample_rate() * self.oversampling_factor() as usize
    }
}

impl ParamBackend for StandaloneBackend {
    fn set_parameter(&self, stage_idx: usize, name: &'static str, value: f32) {
        self.manager.engine().set_parameter(stage_idx, name, value);
    }

    fn rebuild_stage(&self, stage_idx: usize, config: &StageConfig) {
        let sr = self.effective_sample_rate() as f32;
        let runtime_stage = config.to_runtime(sr);
        self.manager
            .engine()
            .replace_stage(stage_idx, runtime_stage);
    }

    fn set_amp_chain(&self, stages: &[StageConfig]) {
        let sr = self.effective_sample_rate();
        let mut chain = AmplifierChain::new();
        for cfg in stages {
            chain.add_stage(cfg.to_runtime(sr as f32));
        }
        for (i, cfg) in stages.iter().enumerate() {
            if cfg.bypassed() {
                chain.set_bypassed(i, true);
            }
        }
        self.manager.engine().set_amp_chain(chain);
    }

    fn set_bypass(&self, stage_idx: usize, bypassed: bool) {
        self.manager
            .engine()
            .set_stage_bypassed(stage_idx, bypassed);
    }

    fn add_stage(&self, idx: usize, config: &StageConfig) {
        let sr = self.effective_sample_rate() as f32;
        let runtime_stage = config.to_runtime(sr);
        self.manager.engine().add_stage(idx, runtime_stage);
    }

    fn remove_stage(&self, idx: usize) {
        self.manager.engine().remove_stage(idx);
    }

    fn swap_stages(&self, a: usize, b: usize) {
        self.manager.engine().swap_stages(a, b);
    }

    fn set_ir(&self, name: &str) {
        self.manager.request_ir_load(name);
    }

    fn set_ir_bypass(&self, bypassed: bool) {
        self.manager.engine().set_ir_bypass(bypassed);
    }

    fn set_ir_gain(&self, gain: f32) {
        self.manager.engine().set_ir_gain(gain);
    }

    fn set_input_filter(&self, filter: &InputFilterConfig) {
        let sample_rate = self.manager.sample_rate() as f32;
        let hp: Option<Box<dyn Stage>> = if filter.hp_enabled {
            Some(Box::new(FilterStage::new(
                FilterType::Highpass,
                filter.hp_cutoff,
                sample_rate,
            )))
        } else {
            None
        };
        let lp: Option<Box<dyn Stage>> = if filter.lp_enabled {
            Some(Box::new(FilterStage::new(
                FilterType::Lowpass,
                filter.lp_cutoff,
                sample_rate,
            )))
        } else {
            None
        };
        self.manager.engine().set_input_filters(hp, lp);
    }

    fn set_pitch_shift(&self, semitones: i32) {
        self.manager.engine().set_pitch_shift(semitones);
    }

    fn set_oversampling(&self, factor: u32) {
        let sample_rate = self.manager.sample_rate();
        let buffer_size = self.manager.buffer_size();
        match Samplers::new(buffer_size, f64::from(factor), sample_rate) {
            Ok(samplers) => {
                self.manager.engine().set_samplers(samplers);
                self.oversampling_factor.store(factor, Ordering::Relaxed);
            }
            Err(e) => error!("Failed to create samplers for {factor}x oversampling: {e}"),
        }
    }

    fn sample_rate(&self) -> u32 {
        self.manager.sample_rate() as u32
    }

    fn oversampling_factor(&self) -> u32 {
        self.oversampling_factor.load(Ordering::Relaxed)
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn get_available_irs(&self) -> Vec<String> {
        self.manager.get_available_irs()
    }

    fn get_peak_meter_info(&self) -> Option<ExternalEvent> {
        let info = self.manager.peak_meter().get_info();
        let xrun_count = self.manager.xrun_count();
        let cpu_load = self.manager.cpu_load();
        Some(ExternalEvent::PeakMeterUpdate {
            info,
            xrun_count,
            cpu_load,
        })
    }
}
