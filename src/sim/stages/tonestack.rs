use crate::sim::stages::Stage;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// Tone stack models for different amplifier types
#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ToneStackModel {
    Modern,   // Mesa Boogie style
    British,  // Marshall style
    American, // Fender style
    Flat,     // Neutral response
}

// A simple tone stack implementation
pub struct ToneStackStage {
    name: String,
    model: ToneStackModel,
    bass: f32,
    mid: f32,
    treble: f32,
    presence: f32,
    sample_rate: f32,
    // Internal filter states
    bass_lp: f32,
    mid_bp_1: f32,
    mid_bp_2: f32,
    treble_hp: f32,
    presence_hp: f32,
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
            name: name.to_string(),
            model,
            bass: bass.clamp(0.0, 1.0),
            mid: mid.clamp(0.0, 1.0),
            treble: treble.clamp(0.0, 1.0),
            presence: presence.clamp(0.0, 1.0),
            sample_rate,
            bass_lp: 0.0,
            mid_bp_1: 0.0,
            mid_bp_2: 0.0,
            treble_hp: 0.0,
            presence_hp: 0.0,
        }
    }
}

impl Stage for ToneStackStage {
    fn process(&mut self, input: f32) -> f32 {
        // Simplified tone stack implementation - in reality this would be more complex
        // with model-specific frequencies and Q factors

        // Get frequency characteristics based on model
        let (bass_freq, mid_freq, treble_freq, presence_freq) = match self.model {
            ToneStackModel::Modern => (120.0, 800.0, 2200.0, 6000.0), // Mesa Boogie
            ToneStackModel::British => (100.0, 700.0, 2000.0, 5000.0), // Marshall
            ToneStackModel::American => (80.0, 500.0, 1800.0, 4000.0), // Fender
            ToneStackModel::Flat => (100.0, 800.0, 2000.0, 5000.0),   // Neutral
        };

        // Calculate filter coefficients (simplified)
        let bass_alpha =
            (1.0 / self.sample_rate) / ((1.0 / (2.0 * PI * bass_freq)) + (1.0 / self.sample_rate));

        let treble_alpha = (1.0 / (2.0 * PI * treble_freq))
            / ((1.0 / (2.0 * PI * treble_freq)) + (1.0 / self.sample_rate));

        let presence_alpha = (1.0 / (2.0 * PI * presence_freq))
            / ((1.0 / (2.0 * PI * presence_freq)) + (1.0 / self.sample_rate));

        // Simple lowpass for bass
        self.bass_lp = self.bass_lp + bass_alpha * (input - self.bass_lp);

        // Very simplified bandpass for mids (in reality would use biquad)
        let mid_hp = input - self.bass_lp;
        self.mid_bp_1 = (1.0 - treble_alpha) * self.mid_bp_1 + treble_alpha * mid_hp;
        self.mid_bp_2 = mid_hp - self.mid_bp_1;

        // Highpass for treble
        self.treble_hp = treble_alpha * (self.treble_hp + input - self.mid_bp_1);

        // Highpass for presence
        self.presence_hp =
            presence_alpha * (self.presence_hp + input - self.bass_lp - self.mid_bp_2);

        // Mix all bands with their respective levels
        let mut output = 0.0;

        // Apply model-specific EQ curves
        match self.model {
            ToneStackModel::Modern => {
                // Mesa Boogie style - scooped mids, tight bass
                output += self.bass_lp * self.bass * 0.8;
                output += self.mid_bp_2 * self.mid * 0.7; // Slightly reduced mids
                output += self.treble_hp * self.treble * 1.2; // Boosted treble
                output += self.presence_hp * self.presence * 1.3; // Strong presence
            }
            ToneStackModel::British => {
                // Marshall style - mid forward
                output += self.bass_lp * self.bass * 0.9;
                output += self.mid_bp_2 * self.mid * 1.2; // Boosted mids
                output += self.treble_hp * self.treble;
                output += self.presence_hp * self.presence;
            }
            ToneStackModel::American => {
                // Fender style - scooped, bright
                output += self.bass_lp * self.bass * 1.1; // Fuller bass
                output += self.mid_bp_2 * self.mid * 0.8; // Reduced mids
                output += self.treble_hp * self.treble * 1.1;
                output += self.presence_hp * self.presence * 0.9;
            }
            ToneStackModel::Flat => {
                // Neutral response
                output += self.bass_lp * self.bass;
                output += self.mid_bp_2 * self.mid;
                output += self.treble_hp * self.treble;
                output += self.presence_hp * self.presence;
            }
        }

        // Normalize output
        output *= 0.25;

        output
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        let clamped = value.clamp(0.0, 1.0);
        match name {
            "bass" => {
                self.bass = clamped;
                Ok(())
            }
            "mid" => {
                self.mid = clamped;
                Ok(())
            }
            "treble" => {
                self.treble = clamped;
                Ok(())
            }
            "presence" => {
                self.presence = clamped;
                Ok(())
            }
            _ => Err("Unknown parameter name"),
        }
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
