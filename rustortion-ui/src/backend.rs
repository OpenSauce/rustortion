use rustortion_core::audio::peak_meter::PeakMeterInfo;
use rustortion_core::preset::InputFilterConfig;
use rustortion_core::preset::stage_config::StageConfig;

/// Capabilities of the current backend — controls which UI sections render.
#[allow(clippy::struct_excessive_bools)]
pub struct Capabilities {
    pub has_settings_dialog: bool,
    pub has_tuner: bool,
    pub has_recorder: bool,
    pub has_midi_config: bool,
    pub has_jack_settings: bool,
    pub has_preset_management: bool,
}

impl Capabilities {
    pub const fn standalone() -> Self {
        Self {
            has_settings_dialog: true,
            has_tuner: true,
            has_recorder: true,
            has_midi_config: true,
            has_jack_settings: true,
            has_preset_management: true,
        }
    }

    pub const fn plugin() -> Self {
        Self {
            has_settings_dialog: false,
            has_tuner: false,
            has_recorder: false,
            has_midi_config: false,
            has_jack_settings: false,
            has_preset_management: false,
        }
    }
}

/// Events originating outside the GUI.
#[derive(Debug, Clone)]
pub enum ExternalEvent {
    PeakMeterUpdate {
        info: PeakMeterInfo,
        xrun_count: u64,
        cpu_load: f32,
    },
    ParamsChanged,
}

/// Trait abstracting how the GUI communicates with the audio engine.
/// Implemented by `StandaloneBackend` (over `Manager`/`Engine`) and
/// `PluginBackend` (over `GuiContext`).
pub trait ParamBackend: Send + Sync + 'static {
    fn set_parameter(&self, stage_idx: usize, name: &'static str, value: f32);
    fn begin_edit(&self, _stage_idx: usize, _name: &str) {}
    fn end_edit(&self, _stage_idx: usize, _name: &str) {}

    fn rebuild_stage(&self, stage_idx: usize, config: &StageConfig);
    fn set_amp_chain(&self, stages: &[StageConfig]);
    fn set_bypass(&self, stage_idx: usize, bypassed: bool);
    fn add_stage(&self, idx: usize, config: &StageConfig);
    fn remove_stage(&self, idx: usize);
    fn swap_stages(&self, a: usize, b: usize);

    fn set_ir(&self, path: &str);
    fn set_ir_bypass(&self, bypassed: bool);
    fn set_ir_gain(&self, gain: f32);

    fn set_input_filter(&self, filter: &InputFilterConfig);
    fn set_pitch_shift(&self, semitones: i32);
    fn set_oversampling(&self, factor: u32);
    fn set_preset_index(&self, _index: usize) {}

    fn sample_rate(&self) -> u32;
    fn oversampling_factor(&self) -> u32;

    fn capabilities(&self) -> &Capabilities;

    fn get_available_irs(&self) -> Vec<String>;
    fn get_peak_meter_info(&self) -> Option<ExternalEvent>;

    /// Called by the shared GUI after any stage mutation (add, remove, reorder,
    /// param change, preset load) so the backend can persist the chain state.
    /// Default is a no-op (standalone doesn't need this).
    fn persist_chain_state(&self, _stages: &[StageConfig]) {}
}
