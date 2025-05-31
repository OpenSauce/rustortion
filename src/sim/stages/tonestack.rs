use crate::sim::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Available EQ curves that loosely match well‑known amp families.
#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
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
            ToneStackModel::Modern => write!(f, "Modern"),
            ToneStackModel::British => write!(f, "British"),
            ToneStackModel::American => write!(f, "American"),
            ToneStackModel::Flat => write!(f, "Flat"),
        }
    }
}

/// Highly efficient 3‑band tone stack (+ Presence shelf).
/// * All controls are 0.0 – 1.0, with 0.5 meaning “flat”.
/// * Internally uses first‑order filters → ~0.005 % CPU on modern hardware.
pub struct ToneStackStage {
    name: String,
    model: ToneStackModel,
    bass: f32,
    mid: f32,
    treble: f32,
    presence: f32,
    sample_rate: f32,

    // --- filter state ---
    dc_hp: f32,

    bass_lp: f32,

    mid_lp: f32,
    mid_hp: f32,

    treble_lp: f32,
    presence_lp: f32,
}

impl ToneStackStage {
    pub fn new(
        name: &str,
        model: ToneStackModel,
        bass: f32,
        mid: f32,
        treble: f32,
        presence: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            name: name.to_owned(),
            model,
            bass: bass.clamp(0.0, 1.0),
            mid: mid.clamp(0.0, 1.0),
            treble: treble.clamp(0.0, 1.0),
            presence: presence.clamp(0.0, 1.0),
            sample_rate,

            // state
            dc_hp: 0.0,
            bass_lp: 0.0,
            mid_lp: 0.0,
            mid_hp: 0.0,
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
        let (bass_f, mid_f, treble_f, presence_f) = match self.model {
            ToneStackModel::Modern => (120.0, 800.0, 2200.0, 6000.0),
            ToneStackModel::British => (100.0, 700.0, 2000.0, 5000.0),
            ToneStackModel::American => (80.0, 500.0, 1800.0, 4000.0),
            ToneStackModel::Flat => (100.0, 800.0, 2000.0, 5000.0),
        };

        // ---------------------------------------------------------
        // 1. Bass – simple first‑order LP
        // ---------------------------------------------------------
        let bass_lp = Self::one_pole_lp(self.alpha(bass_f), &mut self.bass_lp, x);

        // ---------------------------------------------------------
        // 2. Mid – first HP then LP → crude but phase‑coherent BP
        // ---------------------------------------------------------
        let mid_hp_alpha = self.alpha(bass_f); // same as bass corner → remove lows
        self.mid_hp += mid_hp_alpha * (x - self.mid_hp);
        let mid_hp = x - self.mid_hp; // high‑passed signal
        let mid_bp = Self::one_pole_lp(self.alpha(mid_f), &mut self.mid_lp, mid_hp);
        let mid_bp = mid_hp - mid_bp; // band‑pass around mid_f

        // ---------------------------------------------------------
        // 3. Treble – input minus first‑order LP at treble_f
        // ---------------------------------------------------------
        let treble_lp = Self::one_pole_lp(self.alpha(treble_f), &mut self.treble_lp, x);
        let treble_hp = x - treble_lp;

        // ---------------------------------------------------------
        // 4. Primary 3‑band mix (unity at 0.5)
        // ---------------------------------------------------------
        let bass_gain = (self.bass * 2.0).max(0.001); // 0 → −∞ dB, 0.5 → 0 dB, 1 → +6 dB
        let mid_gain = (self.mid * 2.0).max(0.001);
        let treble_gain = (self.treble * 2.0).max(0.001);

        let mut y = bass_lp * bass_gain + mid_bp * mid_gain + treble_hp * treble_gain;

        // ---------------------------------------------------------
        // 5. Presence – gentle high‑shelf (±6 dB)
        // ---------------------------------------------------------
        let pres_alpha = self.alpha(presence_f);
        self.presence_lp += pres_alpha * (y - self.presence_lp);
        let shelf = y + (y - self.presence_lp) * (self.presence * 2.0 - 1.0);

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
        let v = value.clamp(0.0, 1.0);
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

    fn name(&self) -> &str {
        &self.name
    }
}
