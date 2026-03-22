use nih_plug::prelude::*;
use rustortion_core::preset::stage_config::StageConfig;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;

// ---------------------------------------------------------------------------
// Per-slot parameter structs
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct PreampSlotParams {
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "bias"]
    pub bias: FloatParam,
    #[id = "clipper_type"]
    pub clipper_type: IntParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for PreampSlotParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                5.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            bias: FloatParam::new(
                "Bias",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            ),
            clipper_type: IntParam::new("Clipper Type", 0, IntRange::Linear { min: 0, max: 5 })
                .with_value_to_string(Arc::new(|v| {
                    match v {
                        0 => "Soft",
                        1 => "Medium",
                        2 => "Hard",
                        3 => "Asymmetric",
                        4 => "ClassA",
                        5 => "Triode",
                        _ => "Unknown",
                    }
                    .to_string()
                })),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct CompressorSlotParams {
    #[id = "attack_ms"]
    pub attack_ms: FloatParam,
    #[id = "release_ms"]
    pub release_ms: FloatParam,
    #[id = "threshold_db"]
    pub threshold_db: FloatParam,
    #[id = "ratio"]
    pub ratio: FloatParam,
    #[id = "makeup_db"]
    pub makeup_db: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for CompressorSlotParams {
    fn default() -> Self {
        Self {
            attack_ms: FloatParam::new(
                "Attack",
                1.0,
                FloatRange::Linear {
                    min: 0.1,
                    max: 100.0,
                },
            )
            .with_unit(" ms"),
            release_ms: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Linear {
                    min: 10.0,
                    max: 1000.0,
                },
            )
            .with_unit(" ms"),
            threshold_db: FloatParam::new(
                "Threshold",
                -20.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB"),
            ratio: FloatParam::new(
                "Ratio",
                4.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 20.0,
                },
            ),
            makeup_db: FloatParam::new(
                "Makeup",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB"),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct ToneStackSlotParams {
    #[id = "model"]
    pub model: IntParam,
    #[id = "bass"]
    pub bass: FloatParam,
    #[id = "mid"]
    pub mid: FloatParam,
    #[id = "treble"]
    pub treble: FloatParam,
    #[id = "presence"]
    pub presence: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for ToneStackSlotParams {
    fn default() -> Self {
        Self {
            model: IntParam::new("Model", 0, IntRange::Linear { min: 0, max: 3 })
                .with_value_to_string(Arc::new(|v| {
                    match v {
                        0 => "Modern",
                        1 => "British",
                        2 => "American",
                        3 => "Flat",
                        _ => "Unknown",
                    }
                    .to_string()
                })),
            bass: FloatParam::new("Bass", 0.5, FloatRange::Linear { min: 0.0, max: 2.0 }),
            mid: FloatParam::new("Mid", 0.5, FloatRange::Linear { min: 0.0, max: 2.0 }),
            treble: FloatParam::new("Treble", 0.5, FloatRange::Linear { min: 0.0, max: 2.0 }),
            presence: FloatParam::new("Presence", 0.5, FloatRange::Linear { min: 0.0, max: 2.0 }),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct PowerAmpSlotParams {
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "amp_type"]
    pub amp_type: IntParam,
    #[id = "sag"]
    pub sag: FloatParam,
    #[id = "sag_release"]
    pub sag_release: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for PowerAmpSlotParams {
    fn default() -> Self {
        Self {
            drive: FloatParam::new("Drive", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            amp_type: IntParam::new("Amp Type", 1, IntRange::Linear { min: 0, max: 2 })
                .with_value_to_string(Arc::new(|v| {
                    match v {
                        0 => "Class A",
                        1 => "Class AB",
                        2 => "Class B",
                        _ => "Unknown",
                    }
                    .to_string()
                })),
            sag: FloatParam::new("Sag", 0.3, FloatRange::Linear { min: 0.0, max: 1.0 }),
            sag_release: FloatParam::new(
                "Sag Release",
                120.0,
                FloatRange::Linear {
                    min: 40.0,
                    max: 200.0,
                },
            )
            .with_unit(" ms"),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct LevelSlotParams {
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for LevelSlotParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 }),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct NoiseGateSlotParams {
    #[id = "threshold_db"]
    pub threshold_db: FloatParam,
    #[id = "ratio"]
    pub ratio: FloatParam,
    #[id = "attack_ms"]
    pub attack_ms: FloatParam,
    #[id = "hold_ms"]
    pub hold_ms: FloatParam,
    #[id = "release_ms"]
    pub release_ms: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for NoiseGateSlotParams {
    fn default() -> Self {
        Self {
            threshold_db: FloatParam::new(
                "Threshold",
                -40.0,
                FloatRange::Linear {
                    min: -80.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB"),
            ratio: FloatParam::new(
                "Ratio",
                10.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 100.0,
                },
            ),
            attack_ms: FloatParam::new(
                "Attack",
                1.0,
                FloatRange::Linear {
                    min: 0.1,
                    max: 100.0,
                },
            )
            .with_unit(" ms"),
            hold_ms: FloatParam::new(
                "Hold",
                10.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 500.0,
                },
            )
            .with_unit(" ms"),
            release_ms: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 1000.0,
                },
            )
            .with_unit(" ms"),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct MultibandSaturatorSlotParams {
    #[id = "low_drive"]
    pub low_drive: FloatParam,
    #[id = "mid_drive"]
    pub mid_drive: FloatParam,
    #[id = "high_drive"]
    pub high_drive: FloatParam,
    #[id = "low_level"]
    pub low_level: FloatParam,
    #[id = "mid_level"]
    pub mid_level: FloatParam,
    #[id = "high_level"]
    pub high_level: FloatParam,
    #[id = "low_freq"]
    pub low_freq: FloatParam,
    #[id = "high_freq"]
    pub high_freq: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for MultibandSaturatorSlotParams {
    fn default() -> Self {
        Self {
            low_drive: FloatParam::new("Low Drive", 0.3, FloatRange::Linear { min: 0.0, max: 1.0 }),
            mid_drive: FloatParam::new("Mid Drive", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            high_drive: FloatParam::new(
                "High Drive",
                0.4,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            low_level: FloatParam::new("Low Level", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 }),
            mid_level: FloatParam::new("Mid Level", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 }),
            high_level: FloatParam::new(
                "High Level",
                1.0,
                FloatRange::Linear { min: 0.0, max: 2.0 },
            ),
            low_freq: FloatParam::new(
                "Low Freq",
                200.0,
                FloatRange::Linear {
                    min: 50.0,
                    max: 500.0,
                },
            )
            .with_unit(" Hz"),
            high_freq: FloatParam::new(
                "High Freq",
                2500.0,
                FloatRange::Linear {
                    min: 1000.0,
                    max: 6000.0,
                },
            )
            .with_unit(" Hz"),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct DelaySlotParams {
    #[id = "delay_ms"]
    pub delay_ms: FloatParam,
    #[id = "feedback"]
    pub feedback: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for DelaySlotParams {
    fn default() -> Self {
        Self {
            delay_ms: FloatParam::new(
                "Delay",
                300.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 2000.0,
                },
            )
            .with_unit(" ms"),
            feedback: FloatParam::new(
                "Feedback",
                0.3,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.95,
                },
            ),
            mix: FloatParam::new("Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct ReverbSlotParams {
    #[id = "room_size"]
    pub room_size: FloatParam,
    #[id = "damping"]
    pub damping: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for ReverbSlotParams {
    fn default() -> Self {
        Self {
            room_size: FloatParam::new("Room Size", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            damping: FloatParam::new("Damping", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            mix: FloatParam::new("Mix", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 }),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

#[derive(Params)]
pub struct EqSlotParams {
    #[id = "band_0"]
    pub band_0: FloatParam,
    #[id = "band_1"]
    pub band_1: FloatParam,
    #[id = "band_2"]
    pub band_2: FloatParam,
    #[id = "band_3"]
    pub band_3: FloatParam,
    #[id = "band_4"]
    pub band_4: FloatParam,
    #[id = "band_5"]
    pub band_5: FloatParam,
    #[id = "band_6"]
    pub band_6: FloatParam,
    #[id = "band_7"]
    pub band_7: FloatParam,
    #[id = "band_8"]
    pub band_8: FloatParam,
    #[id = "band_9"]
    pub band_9: FloatParam,
    #[id = "band_10"]
    pub band_10: FloatParam,
    #[id = "band_11"]
    pub band_11: FloatParam,
    #[id = "band_12"]
    pub band_12: FloatParam,
    #[id = "band_13"]
    pub band_13: FloatParam,
    #[id = "band_14"]
    pub band_14: FloatParam,
    #[id = "band_15"]
    pub band_15: FloatParam,
    #[id = "bypassed"]
    pub bypassed: BoolParam,
}

impl Default for EqSlotParams {
    fn default() -> Self {
        let eq_range = FloatRange::Linear {
            min: -12.0,
            max: 12.0,
        };
        Self {
            band_0: FloatParam::new("Band 0", 0.0, eq_range).with_unit(" dB"),
            band_1: FloatParam::new("Band 1", 0.0, eq_range).with_unit(" dB"),
            band_2: FloatParam::new("Band 2", 0.0, eq_range).with_unit(" dB"),
            band_3: FloatParam::new("Band 3", 0.0, eq_range).with_unit(" dB"),
            band_4: FloatParam::new("Band 4", 0.0, eq_range).with_unit(" dB"),
            band_5: FloatParam::new("Band 5", 0.0, eq_range).with_unit(" dB"),
            band_6: FloatParam::new("Band 6", 0.0, eq_range).with_unit(" dB"),
            band_7: FloatParam::new("Band 7", 0.0, eq_range).with_unit(" dB"),
            band_8: FloatParam::new("Band 8", 0.0, eq_range).with_unit(" dB"),
            band_9: FloatParam::new("Band 9", 0.0, eq_range).with_unit(" dB"),
            band_10: FloatParam::new("Band 10", 0.0, eq_range).with_unit(" dB"),
            band_11: FloatParam::new("Band 11", 0.0, eq_range).with_unit(" dB"),
            band_12: FloatParam::new("Band 12", 0.0, eq_range).with_unit(" dB"),
            band_13: FloatParam::new("Band 13", 0.0, eq_range).with_unit(" dB"),
            band_14: FloatParam::new("Band 14", 0.0, eq_range).with_unit(" dB"),
            band_15: FloatParam::new("Band 15", 0.0, eq_range).with_unit(" dB"),
            bypassed: BoolParam::new("Bypassed", false),
        }
    }
}

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

    // Per-stage slot arrays
    #[nested(array, group = "Preamp")]
    pub preamp: [PreampSlotParams; 8],

    #[nested(array, group = "Compressor")]
    pub compressor: [CompressorSlotParams; 8],

    #[nested(array, group = "ToneStack")]
    pub tonestack: [ToneStackSlotParams; 8],

    #[nested(array, group = "PowerAmp")]
    pub poweramp: [PowerAmpSlotParams; 8],

    #[nested(array, group = "Level")]
    pub level: [LevelSlotParams; 8],

    #[nested(array, group = "NoiseGate")]
    pub noise_gate: [NoiseGateSlotParams; 8],

    #[nested(array, group = "MultibandSaturator")]
    pub multiband_saturator: [MultibandSaturatorSlotParams; 8],

    #[nested(array, group = "Delay")]
    pub delay: [DelaySlotParams; 8],

    #[nested(array, group = "Reverb")]
    pub reverb: [ReverbSlotParams; 8],

    #[nested(array, group = "EQ")]
    pub eq: [EqSlotParams; 8],
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

            preamp: Default::default(),
            compressor: Default::default(),
            tonestack: Default::default(),
            poweramp: Default::default(),
            level: Default::default(),
            noise_gate: Default::default(),
            multiband_saturator: Default::default(),
            delay: Default::default(),
            reverb: Default::default(),
            eq: Default::default(),
        }
    }
}
