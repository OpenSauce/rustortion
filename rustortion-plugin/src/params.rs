use nih_plug::prelude::*;
use rustortion_core::preset::stage_config::StageConfig;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;

// ---------------------------------------------------------------------------
// Main plugin parameters
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct RustortionParams {
    // Global parameters
    #[id = "output_level"]
    pub output_level: FloatParam,

    #[id = "ir_gain"]
    pub ir_gain: FloatParam,

    #[id = "ir_bypass"]
    pub ir_bypass: BoolParam,

    #[id = "pitch_shift"]
    pub pitch_shift: IntParam,

    #[id = "hp_enabled"]
    pub hp_enabled: BoolParam,

    #[id = "hp_cutoff"]
    pub hp_cutoff: FloatParam,

    #[id = "lp_enabled"]
    pub lp_enabled: BoolParam,

    #[id = "lp_cutoff"]
    pub lp_cutoff: FloatParam,

    #[id = "preset_idx"]
    pub preset_idx: IntParam,

    #[persist = "oversampling_factor"]
    pub oversampling_factor: Arc<AtomicU32>,

    /// Serialized stage chain — persisted with DAW project state so user
    /// modifications (add/remove/reorder stages) survive save/restore.
    #[persist = "chain_state"]
    pub chain_state: Arc<Mutex<Option<Vec<StageConfig>>>>,
}

impl Default for RustortionParams {
    fn default() -> Self {
        Self {
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

            ir_bypass: BoolParam::new("IR Bypass", false),

            pitch_shift: IntParam::new("Pitch Shift", 0, IntRange::Linear { min: -24, max: 24 })
                .with_unit(" st"),

            hp_enabled: BoolParam::new("HP Enabled", true),

            hp_cutoff: FloatParam::new(
                "HP Cutoff",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1000.0,
                },
            )
            .with_unit(" Hz"),

            lp_enabled: BoolParam::new("LP Enabled", true),

            lp_cutoff: FloatParam::new(
                "LP Cutoff",
                8000.0,
                FloatRange::Linear {
                    min: 1000.0,
                    max: 20000.0,
                },
            )
            .with_unit(" Hz"),

            preset_idx: IntParam::new("Preset", 0, IntRange::Linear { min: 0, max: 255 })
                .non_automatable(),

            oversampling_factor: Arc::new(AtomicU32::new(1)), // 1 = 1x (no oversampling)
            chain_state: Arc::new(Mutex::new(None)),
        }
    }
}
