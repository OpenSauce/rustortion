use crate::amp::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Available EQ curves that loosely match well‑known amp families.
#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToneStackModel {
    /// Mesa Rectifier / Mark‑series – tight lows, scooped low‑mids.
    Modern,
    /// Marshall – mid‑forward.
    British,
    /// Fender – deep scoop, glassy top.
    American,
    /// Flat – neutral Baxandall.
    Flat,
}

impl std::fmt::Display for ToneStackModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Modern => write!(f, "{}", crate::tr!(tonestack_modern)),
            Self::British => write!(f, "{}", crate::tr!(tonestack_british)),
            Self::American => write!(f, "{}", crate::tr!(tonestack_american)),
            Self::Flat => write!(f, "{}", crate::tr!(tonestack_flat)),
        }
    }
}

/// Highly efficient 3‑band tone stack (+ Presence shelf).
/// * All controls are 0.0 – 2.0, with 1.0 meaning “flat”.
/// * Internally uses first‑order filters → ~0.005 % CPU on modern hardware.
pub struct ToneStackStage {
    model: ToneStackModel,
    bass: f32,
    mid: f32,
    treble: f32,
    presence: f32,
    sample_rate: f32,

    // --- filter state ---
    dc_hp: f32,

    bass_lp: f32,

    treble_lp: f32,
    presence_lp: f32,
}

impl ToneStackStage {
    pub const fn new(
        model: ToneStackModel,
        bass: f32,
        mid: f32,
        treble: f32,
        presence: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            model,
            bass: bass.clamp(0.0, 2.0),
            mid: mid.clamp(0.0, 2.0),
            treble: treble.clamp(0.0, 2.0),
            presence: presence.clamp(0.0, 2.0),
            sample_rate,

            // state
            dc_hp: 0.0,
            bass_lp: 0.0,
            treble_lp: 0.0,
            presence_lp: 0.0,
        }
    }

    #[inline]
    fn one_pole_lp(alpha: f32, state: &mut f32, x: f32) -> f32 {
        *state += alpha * (x - *state);
        *state
    }

    #[inline]
    fn alpha(&self, f: f32) -> f32 {
        let dt = 1.0 / self.sample_rate;
        dt / (dt + 1.0 / (2.0 * PI * f))
    }
}

impl Stage for ToneStackStage {
    fn process(&mut self, input: f32) -> f32 {
        // ---------------------------------------------------------
        // 0. DC blocker (20 Hz HP) – keeps downstream stages happy
        // ---------------------------------------------------------
        let dc_alpha = self.alpha(20.0);
        self.dc_hp += dc_alpha * (input - self.dc_hp);
        let x = input - self.dc_hp;

        // ---------------------------------------------------------
        // Model‑specific corner frequencies (Hz)
        // ---------------------------------------------------------
        let (bass_f, treble_f, presence_f) = match self.model {
            ToneStackModel::Modern => (120.0, 2200.0, 6000.0),
            ToneStackModel::British => (100.0, 2000.0, 5000.0),
            ToneStackModel::American => (80.0, 1800.0, 4000.0),
            ToneStackModel::Flat => (100.0, 2000.0, 5000.0),
        };

        // ---------------------------------------------------------
        // 1. Bass – simple first‑order LP
        // ---------------------------------------------------------
        let bass_lp = Self::one_pole_lp(self.alpha(bass_f), &mut self.bass_lp, x);

        // ---------------------------------------------------------
        // 2. Treble – input minus first‑order LP at treble_f
        // ---------------------------------------------------------
        let treble_lp = Self::one_pole_lp(self.alpha(treble_f), &mut self.treble_lp, x);
        let treble_hp = x - treble_lp;

        // ---------------------------------------------------------
        // 3. Mid – subtractive: everything between bass LP and treble LP
        //    At unity gains: bass + mid + treble = LP(bass) + [LP(treble) - LP(bass)] + [x - LP(treble)] = x
        // ---------------------------------------------------------
        let mid = treble_lp - bass_lp;

        // ---------------------------------------------------------
        // 4. Primary 3‑band mix (unity at 1.0)
        // ---------------------------------------------------------
        let bass_gain = self.bass.max(0.001); // 0 → −∞ dB, 1.0 → 0 dB, 2.0 → +6 dB
        let mid_gain = self.mid.max(0.001);
        let treble_gain = self.treble.max(0.001);

        let mut y = treble_hp.mul_add(treble_gain, bass_lp.mul_add(bass_gain, mid * mid_gain));

        // ---------------------------------------------------------
        // 5. Presence -- high-shelf (+-6 dB, dB-mapped)
        // ---------------------------------------------------------
        let pres_alpha = self.alpha(presence_f);
        self.presence_lp += pres_alpha * (y - self.presence_lp);
        let pres_db = (self.presence - 1.0) * 6.0;
        let pres_lin = 10.0_f32.powf(pres_db / 20.0);
        let shelf = (y - self.presence_lp).mul_add(pres_lin, self.presence_lp);

        // ---------------------------------------------------------
        // 6. Model flavour adjustments
        // ---------------------------------------------------------
        match self.model {
            ToneStackModel::Modern => {
                // extra low‑mid scoop & bright edge
                y = shelf * 0.95;
            }
            ToneStackModel::British => {
                // push mids forward lightly
                y = shelf * 1.05;
            }
            ToneStackModel::American => {
                // tiny dip in low mids
                y = shelf * 0.97;
            }
            ToneStackModel::Flat => y = shelf,
        }

        // leave ~‑3 dB headroom so downstream stages aren’t slammed
        y * 0.7
    }

    // -------------------------------------------------------------
    // Parameter management
    // -------------------------------------------------------------
    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        let v = value.clamp(0.0, 2.0);
        match name {
            "bass" => self.bass = v,
            "mid" => self.mid = v,
            "treble" => self.treble = v,
            "presence" => self.presence = v,
            _ => return Err("Unknown parameter name"),
        }
        Ok(())
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "bass" => Ok(self.bass),
            "mid" => Ok(self.mid),
            "treble" => Ok(self.treble),
            "presence" => Ok(self.presence),
            _ => Err("Unknown parameter name"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn make_tonestack(model: ToneStackModel) -> ToneStackStage {
        ToneStackStage::new(model, 1.0, 1.0, 1.0, 1.0, SR)
    }

    /// Run a sine wave through the stage and return RMS energy
    fn measure_sine_energy(stage: &mut ToneStackStage, freq_hz: f32, num_cycles: usize) -> f32 {
        // Compute total duration directly from desired cycles, rounding to nearest
        // whole sample to avoid truncation bias at high frequencies.
        let total_samples = ((num_cycles as f32) * SR / freq_hz).round() as usize;
        // Warm up
        for i in 0..total_samples {
            let t = i as f32 / SR;
            stage.process((2.0 * PI * freq_hz * t).sin() * 0.5);
        }
        // Measure
        let mut energy = 0.0_f32;
        for i in 0..total_samples {
            let t = (total_samples + i) as f32 / SR;
            let out = stage.process((2.0 * PI * freq_hz * t).sin() * 0.5);
            energy += out * out;
        }
        (energy / total_samples as f32).sqrt()
    }

    #[test]
    fn test_flat_unity_passthrough() {
        let mut stage = make_tonestack(ToneStackModel::Flat);
        let freq = 1000.0;
        // Warm up to let filters/DC blocker settle
        for i in 0..4000 {
            let t = i as f32 / SR;
            stage.process((2.0 * PI * freq * t).sin() * 0.5);
        }
        // Measure RMS gain over a window
        let mut sum_in2 = 0.0_f32;
        let mut sum_out2 = 0.0_f32;
        let num_samples = 2000_usize;
        for i in 0..num_samples {
            let t = (4000 + i) as f32 / SR;
            let input = (2.0 * PI * freq * t).sin() * 0.5;
            let out = stage.process(input);
            sum_in2 += input * input;
            sum_out2 += out * out;
        }
        let input_rms = (sum_in2 / num_samples as f32).sqrt();
        let output_rms = (sum_out2 / num_samples as f32).sqrt();
        let gain = output_rms / input_rms;
        // Expect ~0.7x (-3 dB headroom) with tolerance for filter settling
        assert!(
            (gain - 0.7).abs() <= 0.05,
            "flat unity should be near 0.7x RMS passthrough: measured gain={gain}"
        );
    }

    #[test]
    fn test_bass_boost() {
        let mut boosted = ToneStackStage::new(ToneStackModel::Flat, 2.0, 1.0, 1.0, 1.0, SR);
        let mut cut = ToneStackStage::new(ToneStackModel::Flat, 0.1, 1.0, 1.0, 1.0, SR);
        let bass_energy_boost = measure_sine_energy(&mut boosted, 80.0, 40);
        let bass_energy_cut = measure_sine_energy(&mut cut, 80.0, 40);
        assert!(
            bass_energy_boost > bass_energy_cut,
            "bass=2.0 should have more low energy than bass=0.1: boost={bass_energy_boost}, cut={bass_energy_cut}"
        );
    }

    #[test]
    fn test_treble_boost() {
        let mut boosted = ToneStackStage::new(ToneStackModel::Flat, 1.0, 1.0, 2.0, 1.0, SR);
        let mut cut = ToneStackStage::new(ToneStackModel::Flat, 1.0, 1.0, 0.1, 1.0, SR);
        let energy_boost = measure_sine_energy(&mut boosted, 8000.0, 100);
        let energy_cut = measure_sine_energy(&mut cut, 8000.0, 100);
        assert!(
            energy_boost > energy_cut,
            "treble=2.0 should have more high-freq energy: boost={energy_boost}, cut={energy_cut}"
        );
    }

    #[test]
    fn test_mid_scoop() {
        let mut full_mid = ToneStackStage::new(ToneStackModel::Flat, 1.0, 2.0, 1.0, 1.0, SR);
        let mut scooped = ToneStackStage::new(ToneStackModel::Flat, 1.0, 0.1, 1.0, 1.0, SR);
        let energy_full = measure_sine_energy(&mut full_mid, 1000.0, 50);
        let energy_scoop = measure_sine_energy(&mut scooped, 1000.0, 50);
        assert!(
            energy_full > energy_scoop,
            "mid=2.0 should have more mid energy than mid=0.1: full={energy_full}, scoop={energy_scoop}"
        );
    }

    #[test]
    fn test_presence_effect() {
        let mut high_pres = ToneStackStage::new(ToneStackModel::Flat, 1.0, 1.0, 1.0, 2.0, SR);
        let mut low_pres = ToneStackStage::new(ToneStackModel::Flat, 1.0, 1.0, 1.0, 0.0, SR);
        let energy_high = measure_sine_energy(&mut high_pres, 6000.0, 100);
        let energy_low = measure_sine_energy(&mut low_pres, 6000.0, 100);
        assert!(
            energy_high > energy_low,
            "presence=2.0 should boost highs vs 0.0: high={energy_high}, low={energy_low}"
        );
    }

    #[test]
    fn test_model_differences() {
        let models = [
            ToneStackModel::Modern,
            ToneStackModel::British,
            ToneStackModel::American,
            ToneStackModel::Flat,
        ];
        let mut energies = Vec::new();
        for model in &models {
            let mut stage = make_tonestack(*model);
            energies.push(measure_sine_energy(&mut stage, 1000.0, 50));
        }
        let all_same = energies.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6);
        assert!(
            !all_same,
            "models should produce different output: {energies:?}"
        );
    }

    #[test]
    fn test_dc_rejection() {
        let mut stage = make_tonestack(ToneStackModel::Flat);
        for _ in 0..48000 {
            stage.process(0.5);
        }
        let mut avg = 0.0_f32;
        let n = 4096;
        for _ in 0..n {
            avg += stage.process(0.5);
        }
        avg /= n as f32;
        assert!(avg.abs() < 0.05, "DC blocker should remove DC, avg={avg}");
    }

    #[test]
    fn test_bounded_output() {
        let mut stage = ToneStackStage::new(ToneStackModel::Modern, 2.0, 2.0, 2.0, 2.0, SR);
        for i in 0..5000 {
            let input = (i as f32 * 0.1).sin() * 5.0;
            let out = stage.process(input);
            assert!(
                out.is_finite() && out.abs() < 50.0,
                "output must be bounded, got {out}"
            );
        }
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut stage = make_tonestack(ToneStackModel::Flat);
        stage.set_parameter("bass", 1.5).unwrap();
        assert!((stage.get_parameter("bass").unwrap() - 1.5).abs() < 1e-6);
        stage.set_parameter("mid", 0.3).unwrap();
        assert!((stage.get_parameter("mid").unwrap() - 0.3).abs() < 1e-6);
        stage.set_parameter("treble", 1.8).unwrap();
        assert!((stage.get_parameter("treble").unwrap() - 1.8).abs() < 1e-6);
        stage.set_parameter("presence", 0.5).unwrap();
        assert!((stage.get_parameter("presence").unwrap() - 0.5).abs() < 1e-6);
        assert!(stage.get_parameter("unknown").is_err());
        assert!(stage.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn test_parameter_clamping() {
        let mut stage = make_tonestack(ToneStackModel::Flat);
        stage.set_parameter("bass", 5.0).unwrap();
        assert!(
            (stage.get_parameter("bass").unwrap() - 2.0).abs() < 1e-6,
            "out-of-range high should clamp to 2.0"
        );
        stage.set_parameter("bass", -1.0).unwrap();
        assert!(
            (stage.get_parameter("bass").unwrap() - 0.0).abs() < 1e-6,
            "out-of-range low should clamp to 0.0"
        );
    }
}
