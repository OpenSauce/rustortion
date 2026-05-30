use log::warn;
use nam_rs::WaveNet;
use serde::{Deserialize, Serialize};

use crate::amp::stages::Stage;
use crate::amp::stages::common::db_to_lin;
use crate::nam::registry;

/// Valid range for the input/output gain knobs, matching the UI and plugin params.
const GAIN_DB_MIN: f32 = -24.0;
const GAIN_DB_MAX: f32 = 24.0;

/// A Neural Amp Modeler stage running a WaveNet `.nam` model.
///
/// With no model loaded the stage is a passthrough. Input/output gain are applied
/// around the model and the wet output is blended with the dry signal via `mix`.
pub struct NamStage {
    wavenet: Option<WaveNet>,
    input_gain: f32,
    output_gain: f32,
    mix: f32,
    /// Native sample rate of the loaded model (0.0 if none), for UI display.
    native_sample_rate: f32,
    /// True if the model's native rate differs from the engine rate.
    sample_rate_mismatch: bool,
}

impl NamStage {
    const fn passthrough(input_gain: f32, output_gain: f32, mix: f32) -> Self {
        Self {
            wavenet: None,
            input_gain,
            output_gain,
            mix,
            native_sample_rate: 0.0,
            sample_rate_mismatch: false,
        }
    }
}

impl Stage for NamStage {
    fn process(&mut self, input: f32) -> f32 {
        let Some(wavenet) = self.wavenet.as_mut() else {
            return input;
        };
        let wet = wavenet.process_sample(input * self.input_gain) * self.output_gain;
        self.mix.mul_add(wet - input, input)
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "input_gain_db" => {
                if (GAIN_DB_MIN..=GAIN_DB_MAX).contains(&value) {
                    self.input_gain = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Input gain must be between -24 and 24 dB")
                }
            }
            "output_gain_db" => {
                if (GAIN_DB_MIN..=GAIN_DB_MAX).contains(&value) {
                    self.output_gain = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Output gain must be between -24 and 24 dB")
                }
            }
            "mix" => {
                if (0.0..=1.0).contains(&value) {
                    self.mix = value;
                    Ok(())
                } else {
                    Err("Mix must be between 0.0 and 1.0")
                }
            }
            "native_sample_rate" | "sample_rate_mismatch" => Err("Parameter is read-only"),
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "input_gain_db" => Ok(20.0 * self.input_gain.log10()),
            "output_gain_db" => Ok(20.0 * self.output_gain.log10()),
            "mix" => Ok(self.mix),
            "native_sample_rate" => Ok(self.native_sample_rate),
            "sample_rate_mismatch" => Ok(f32::from(u8::from(self.sample_rate_mismatch))),
            _ => Err("Unknown parameter name"),
        }
    }
}

// --- Config ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamConfig {
    /// Display name of the selected model, or `None` for passthrough.
    #[serde(default)]
    pub model_name: Option<String>,
    pub input_gain_db: f32,
    pub output_gain_db: f32,
    pub mix: f32,
    #[serde(default)]
    pub bypassed: bool,
}

impl Default for NamConfig {
    fn default() -> Self {
        Self {
            model_name: None,
            input_gain_db: 0.0,
            output_gain_db: 0.0,
            mix: 1.0,
            bypassed: false,
        }
    }
}

impl NamConfig {
    /// Build a runnable stage. Resolves the model from the global registry and
    /// allocates the `WaveNet` here (off the real-time thread). On any failure the
    /// stage falls back to passthrough with a warning.
    pub fn to_stage(&self, sample_rate: f32) -> NamStage {
        let input_gain = db_to_lin(self.input_gain_db.clamp(GAIN_DB_MIN, GAIN_DB_MAX));
        let output_gain = db_to_lin(self.output_gain_db.clamp(GAIN_DB_MIN, GAIN_DB_MAX));
        let mix = self.mix.clamp(0.0, 1.0);

        let Some(name) = self.model_name.as_deref() else {
            return NamStage::passthrough(input_gain, output_gain, mix);
        };

        let Some(model) = registry::get(name) else {
            warn!("NAM model '{name}' not found in registry; using passthrough");
            return NamStage::passthrough(input_gain, output_gain, mix);
        };

        let native_sample_rate = model.sample_rate() as f32;
        let sample_rate_mismatch = (native_sample_rate - sample_rate).abs() > 1.0;
        if sample_rate_mismatch {
            warn!(
                "NAM model '{name}' native rate {native_sample_rate} Hz differs from engine \
                 rate {sample_rate} Hz; tone may be affected"
            );
        }

        match WaveNet::new(&model) {
            Ok(wavenet) => NamStage {
                wavenet: Some(wavenet),
                input_gain,
                output_gain,
                mix,
                native_sample_rate,
                sample_rate_mismatch,
            },
            Err(e) => {
                warn!("Failed to build NAM model '{name}': {e}; using passthrough");
                NamStage::passthrough(input_gain, output_gain, mix)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_no_model() {
        let stage = NamConfig::default().to_stage(48_000.0);
        let mut stage = stage;
        for x in [-1.0, 0.0, 0.25, 0.9] {
            assert_eq!(stage.process(x), x);
        }
    }

    #[test]
    fn gain_and_mix_round_trip() {
        let mut stage = NamConfig::default().to_stage(48_000.0);
        stage.set_parameter("mix", 0.5).unwrap();
        assert!((stage.get_parameter("mix").unwrap() - 0.5).abs() < 1e-6);

        stage.set_parameter("input_gain_db", 6.0).unwrap();
        assert!((stage.get_parameter("input_gain_db").unwrap() - 6.0).abs() < 1e-3);

        assert!(stage.set_parameter("mix", 2.0).is_err());
        assert!(stage.set_parameter("native_sample_rate", 1.0).is_err());

        // Gains outside ±24 dB (and NaN) are rejected.
        assert!(stage.set_parameter("input_gain_db", 30.0).is_err());
        assert!(stage.set_parameter("output_gain_db", -30.0).is_err());
        assert!(stage.set_parameter("input_gain_db", f32::NAN).is_err());
    }
}
