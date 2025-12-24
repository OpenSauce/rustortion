use crate::sim::stages::Stage;

pub struct CompressorStage {
    attack: f32,      // Attack coefficient (0-1)
    release: f32,     // Release coefficient (0-1)
    attack_ms: f32,   // Attack time in milliseconds
    release_ms: f32,  // Release time in milliseconds
    threshold: f32,   // Threshold in linear scale
    ratio: f32,       // Compression ratio (e.g., 4.0 for 4:1)
    makeup: f32,      // Makeup gain in linear scale
    envelope: f32,    // Envelope follower state
    sample_rate: f32, // Sample rate for recalculating coefficients
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl CompressorStage {
    pub fn new(
        attack_ms: f32,
        release_ms: f32,
        threshold_db: f32,
        ratio: f32,
        makeup_db: f32,
        sample_rate: f32,
    ) -> Self {
        // Convert ms to one-pole coefficients
        // For attack/release: smaller time constant = faster response = smaller coefficient
        let attack = (-1.0 / (sample_rate * 0.001 * attack_ms)).exp();
        let release = (-1.0 / (sample_rate * 0.001 * release_ms)).exp();

        Self {
            attack,
            release,
            attack_ms,
            release_ms,
            threshold: db_to_lin(threshold_db),
            ratio,
            makeup: db_to_lin(makeup_db),
            envelope: 0.0,
            sample_rate,
        }
    }

    // Calculate coefficient from time constant in ms
    fn calculate_coefficient(&self, time_ms: f32) -> f32 {
        (-1.0 / (self.sample_rate * 0.001 * time_ms)).exp()
    }

    // Update just the attack time and coefficient
    fn update_attack(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms;
        self.attack = self.calculate_coefficient(attack_ms);
    }

    // Update just the release time and coefficient
    fn update_release(&mut self, release_ms: f32) {
        self.release_ms = release_ms;
        self.release = self.calculate_coefficient(release_ms);
    }
}

impl Stage for CompressorStage {
    fn process(&mut self, input: f32) -> f32 {
        // Envelope follower
        let level_in = input.abs().max(1e-10); // Avoid log(0)

        // Attack/release behavior - using proper one-pole filter form:
        // y[n] = α·y[n-1] + (1-α)·x[n]
        //
        // Where α is closer to 1.0 for longer time constants
        // For attack (level_in > envelope): use attack coefficient (faster)
        // For release (level_in < envelope): use release coefficient (slower)
        if level_in > self.envelope {
            // Attack phase - faster coefficient
            self.envelope = self.attack * self.envelope + (1.0 - self.attack) * level_in;
        } else {
            // Release phase - slower coefficient
            self.envelope = self.release * self.envelope + (1.0 - self.release) * level_in;
        }

        // Compression gain calculation
        let over_threshold = (self.envelope / self.threshold).max(1.0);
        let gain_reduction = if over_threshold > 1.0 {
            // G = (in/threshold)^(1/ratio-1)
            over_threshold.powf((1.0 / self.ratio) - 1.0)
        } else {
            1.0
        };

        // Apply compression and makeup gain
        input * gain_reduction * self.makeup
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "threshold" => {
                if (-60.0..=0.).contains(&value) {
                    self.threshold = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Threshold must be between -60 dB and 0 dB")
                }
            }
            "ratio" => {
                if (1.0..=20.0).contains(&value) {
                    self.ratio = value;
                    Ok(())
                } else {
                    Err("Ratio must be between 1.0 and 20.0")
                }
            }
            "attack" => {
                if (0.1..=100.0).contains(&value) {
                    self.update_attack(value);
                    Ok(())
                } else {
                    Err("Attack must be between 0.1 ms and 100 ms")
                }
            }
            "release" => {
                if (10.0..=1000.0).contains(&value) {
                    self.update_release(value);
                    Ok(())
                } else {
                    Err("Release must be between 10 ms and 1000 ms")
                }
            }
            "makeup" => {
                if (-12.0..=24.0).contains(&value) {
                    self.makeup = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Makeup must be between -12 dB and 24 dB")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "threshold" => Ok(20.0 * self.threshold.log10()), // Convert back to dB
            "ratio" => Ok(self.ratio),
            "attack" => Ok(self.attack_ms), // Return stored ms value directly
            "release" => Ok(self.release_ms), // Return stored ms value directly
            "makeup" => Ok(20.0 * self.makeup.log10()), // Convert back to dB
            _ => Err("Unknown parameter name"),
        }
    }
}
