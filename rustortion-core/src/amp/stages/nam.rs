use log::warn;
use nam_rs::Model;
use serde::{Deserialize, Serialize};

use crate::amp::stages::Stage;
use crate::amp::stages::common::db_to_lin;
use crate::nam::registry;

/// Valid range for the input/output gain knobs, matching the UI and plugin params.
const GAIN_DB_MIN: f32 = -24.0;
const GAIN_DB_MAX: f32 = 24.0;

/// A Neural Amp Modeler stage running a `.nam` model of any supported architecture
/// (WaveNet or LSTM), via the architecture-agnostic [`nam_rs::Model`].
///
/// With no model loaded the stage is a passthrough. Input/output gain are applied
/// around the model and the wet output is blended with the dry signal via `mix`.
pub struct NamStage {
    model: Option<Model>,
    input_gain: f32,
    output_gain: f32,
    mix: f32,
    /// Native sample rate of the loaded model (0.0 if none), for UI display.
    native_sample_rate: f32,
    /// True if the model's native rate differs from the engine rate.
    sample_rate_mismatch: bool,
    /// Scratch buffer holding the dry signal during block processing, so the
    /// in-place `process_buffer` output can be blended back with `mix`. Grown on
    /// demand (first block of a given size); steady-state processing never allocates.
    dry: Vec<f32>,
}

impl NamStage {
    const fn passthrough(input_gain: f32, output_gain: f32, mix: f32) -> Self {
        Self {
            model: None,
            input_gain,
            output_gain,
            mix,
            native_sample_rate: 0.0,
            sample_rate_mismatch: false,
            dry: Vec::new(),
        }
    }

    /// Passthrough used when the model's native rate mismatches the engine rate.
    /// Carries the real native rate so the UI/params can report the bypass reason.
    const fn bypassed_for_mismatch(
        input_gain: f32,
        output_gain: f32,
        mix: f32,
        native_sample_rate: f32,
    ) -> Self {
        Self {
            model: None,
            input_gain,
            output_gain,
            mix,
            native_sample_rate,
            sample_rate_mismatch: true,
            dry: Vec::new(),
        }
    }

    /// True when a model is loaded and running (not a passthrough or rate-mismatch bypass).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.model.is_some()
    }
}

impl Stage for NamStage {
    fn process(&mut self, input: f32) -> f32 {
        let Some(model) = self.model.as_mut() else {
            return input;
        };
        let wet = model.process_sample(input * self.input_gain) * self.output_gain;
        self.mix.mul_add(wet - input, input)
    }

    fn process_block(&mut self, input: &mut [f32]) {
        // No model → dry passthrough (matches `process`'s early return).
        if self.model.is_none() {
            return;
        }

        // Stash the dry signal, then scale the buffer by input gain in place so the
        // model's batched `process_buffer` runs over the gained signal. `resize` only
        // allocates the first time a given block size is seen; steady state is alloc-free.
        if self.dry.len() < input.len() {
            self.dry.resize(input.len(), 0.0);
        }
        let dry = &mut self.dry[..input.len()];
        for (d, x) in dry.iter_mut().zip(input.iter_mut()) {
            *d = *x;
            *x *= self.input_gain;
        }

        // Borrow the model only here (after the `self.dry` borrow above is done being set up).
        let model = self.model.as_mut().expect("model present (checked above)");
        model.process_buffer(input);

        // Apply output gain and blend wet/dry per sample — same formula as `process`.
        for (x, &d) in input.iter_mut().zip(self.dry[..].iter()) {
            let wet = *x * self.output_gain;
            *x = self.mix.mul_add(wet - d, d);
        }
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
    /// allocates the model here (off the real-time thread). On any failure the
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

        let native_sample_rate = model.expected_sample_rate() as f32;
        if (native_sample_rate - sample_rate).abs() > 1.0 {
            // Resampling is intentionally avoided (too expensive on the RT path), so a
            // rate mismatch bypasses the model entirely: pass the dry signal through.
            warn!(
                "NAM model '{name}' native rate {native_sample_rate} Hz differs from engine \
                 rate {sample_rate} Hz; bypassing model (dry passthrough)"
            );
            return NamStage::bypassed_for_mismatch(
                input_gain,
                output_gain,
                mix,
                native_sample_rate,
            );
        }

        match Model::from_nam(&model) {
            Ok(runtime) => NamStage {
                model: Some(runtime),
                input_gain,
                output_gain,
                mix,
                native_sample_rate,
                // Rates match (mismatch returned early above).
                sample_rate_mismatch: false,
                dry: Vec::new(),
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
    fn mismatch_bypass_is_dry_passthrough() {
        // A rate-mismatch stage is built without a model but records the real native
        // rate and the mismatch flag. We construct it directly here because building a
        // real model requires loading a `.nam` file into the registry, which unit
        // tests can't do; this still verifies the RT-path passthrough contract and the
        // params reported to the UI.
        let mut stage =
            NamStage::bypassed_for_mismatch(db_to_lin(6.0), db_to_lin(-3.0), 0.5, 44_100.0);

        // No model runs: output is the dry input, with no gain or mix applied.
        for x in [-1.0, 0.0, 0.25, 0.9] {
            assert_eq!(stage.process(x), x);
        }

        assert!((stage.get_parameter("native_sample_rate").unwrap() - 44_100.0).abs() < 1e-3);
        assert!((stage.get_parameter("sample_rate_mismatch").unwrap() - 1.0).abs() < 1e-6);
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

    /// `process_block` (batched `process_buffer` + gain/mix wrapper) must match the
    /// per-sample `process` path bit-for-bit (within float tolerance). Uses the vendored
    /// MIT reference model in `tests/fixtures/`, so this runs in CI.
    #[test]
    fn block_matches_per_sample_with_real_model() {
        use crate::nam::{NamLoader, registry};
        use std::path::Path;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let Ok(loader) = NamLoader::new(&dir) else {
            return;
        };
        registry::init_from_loader(&loader);
        let Some(name) = registry::available_names().into_iter().next() else {
            eprintln!("skipping NAM parity test: no model available");
            return;
        };

        let config = NamConfig {
            model_name: Some(name),
            input_gain_db: 6.0,
            output_gain_db: -3.0,
            mix: 0.5,
            bypassed: false,
        };

        // Two stages from the same config evolve identical internal state given the
        // same input, so per-sample and block paths should agree.
        let mut per_sample = config.to_stage(48_000.0);
        let mut block = config.to_stage(48_000.0);
        if !per_sample.is_active() {
            eprintln!("skipping NAM parity test: model bypassed at 48 kHz");
            return;
        }

        // A non-trivial signal so gain/mix differences would show up.
        let input: Vec<f32> = (0..256)
            .map(|i| {
                let t = i as f32;
                0.3f32.mul_add((t * 0.05).sin(), 0.1 * (t * 0.31).cos())
            })
            .collect();

        let expected: Vec<f32> = input.iter().map(|&x| per_sample.process(x)).collect();
        let mut got = input; // moved: input is not needed after this
        block.process_block(&mut got);

        for (i, (e, g)) in expected.iter().zip(got.iter()).enumerate() {
            assert!(
                (e - g).abs() < 1e-5,
                "mismatch at {i}: per-sample={e}, block={g}"
            );
        }
    }
}
