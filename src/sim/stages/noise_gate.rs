use crate::sim::stages::Stage;

/// Noise gate stage for eliminating unwanted noise when not playing
/// Features:
/// - Threshold: Level below which the gate closes
/// - Ratio: How much to attenuate when closed (not full mute for smoother transitions)
/// - Attack: How fast the gate opens
/// - Hold: How long to stay open after signal drops
/// - Release: How fast the gate closes
pub struct NoiseGateStage {
    threshold: f32,  // Linear scale (converted from dB)
    ratio: f32,      // Reduction ratio when gate is closed (e.g., 10:1)
    attack_ms: f32,  // Attack time in milliseconds
    hold_ms: f32,    // Hold time in milliseconds
    release_ms: f32, // Release time in milliseconds

    // Internal state
    envelope: f32,       // Current envelope level
    gate_state: f32,     // Current gate state (0 = closed, 1 = open)
    hold_counter: usize, // Sample counter for hold time
    sample_rate: f32,

    // Coefficients (calculated from times)
    attack_coeff: f32,
    release_coeff: f32,
    env_attack_coeff: f32,
    env_release_coeff: f32,
}

impl NoiseGateStage {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
        sample_rate: f32,
    ) -> Self {
        let threshold = db_to_lin(threshold_db);

        // Calculate filter coefficients for smooth transitions
        // Using one-pole filter: coeff = exp(-1 / (sample_rate * time_in_seconds))
        let attack_coeff = calculate_coefficient(attack_ms, sample_rate);
        let release_coeff = calculate_coefficient(release_ms, sample_rate);

        // Envelope follower coefficients (faster for tracking input)
        let env_attack_coeff = calculate_coefficient(0.1, sample_rate); // 0.1ms for fast tracking
        let env_release_coeff = calculate_coefficient(10.0, sample_rate); // 10ms for smoother release

        Self {
            threshold,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            envelope: 0.0,
            gate_state: 0.0,
            hold_counter: 0,
            sample_rate,
            attack_coeff,
            release_coeff,
            env_attack_coeff,
            env_release_coeff,
        }
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = calculate_coefficient(self.attack_ms, self.sample_rate);
        self.release_coeff = calculate_coefficient(self.release_ms, self.sample_rate);
    }

    fn get_hold_samples(&self) -> usize {
        ((self.hold_ms * 0.001) * self.sample_rate) as usize
    }
}

impl Stage for NoiseGateStage {
    fn process(&mut self, input: f32) -> f32 {
        // Step 1: Track the input envelope
        let input_abs = input.abs();

        if input_abs > self.envelope {
            // Attack phase - track rising signal quickly
            self.envelope =
                self.env_attack_coeff * self.envelope + (1.0 - self.env_attack_coeff) * input_abs;
        } else {
            // Release phase - track falling signal more slowly
            self.envelope =
                self.env_release_coeff * self.envelope + (1.0 - self.env_release_coeff) * input_abs;
        }

        // Step 2: Determine if gate should be open or closed
        let should_open = self.envelope > self.threshold;

        // Step 3: Handle hold time
        if should_open {
            self.hold_counter = self.get_hold_samples();
        } else if self.hold_counter > 0 {
            self.hold_counter -= 1;
        }

        let target_state = if should_open || self.hold_counter > 0 {
            1.0 // Gate open
        } else {
            0.0 // Gate closed
        };

        // Step 4: Smooth gate state transitions
        if target_state > self.gate_state {
            // Opening (attack)
            self.gate_state =
                self.attack_coeff * self.gate_state + (1.0 - self.attack_coeff) * target_state;
        } else {
            // Closing (release)
            self.gate_state =
                self.release_coeff * self.gate_state + (1.0 - self.release_coeff) * target_state;
        }

        // Step 5: Apply gating with ratio
        // When gate is closed, apply reduction based on ratio
        // ratio of 10:1 means -20dB reduction when closed
        let reduction = if self.gate_state < 0.999 {
            let closed_gain = 1.0 / self.ratio;

            closed_gain + (1.0 - closed_gain) * self.gate_state
        } else {
            1.0
        };

        input * reduction
    }

    fn process_block(&mut self, input: &mut [f32]) {
        for sample in input.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "threshold" => {
                if (-80.0..=0.0).contains(&value) {
                    self.threshold = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Threshold must be between -80 dB and 0 dB")
                }
            }
            "ratio" => {
                if (1.0..=100.0).contains(&value) {
                    self.ratio = value;
                    Ok(())
                } else {
                    Err("Ratio must be between 1:1 and 100:1")
                }
            }
            "attack" => {
                if (0.1..=100.0).contains(&value) {
                    self.attack_ms = value;
                    self.update_coefficients();
                    Ok(())
                } else {
                    Err("Attack must be between 0.1 ms and 100 ms")
                }
            }
            "hold" => {
                if (0.0..=500.0).contains(&value) {
                    self.hold_ms = value;
                    Ok(())
                } else {
                    Err("Hold must be between 0 ms and 500 ms")
                }
            }
            "release" => {
                if (1.0..=1000.0).contains(&value) {
                    self.release_ms = value;
                    self.update_coefficients();
                    Ok(())
                } else {
                    Err("Release must be between 1 ms and 1000 ms")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "threshold" => Ok(20.0 * self.threshold.log10()),
            "ratio" => Ok(self.ratio),
            "attack" => Ok(self.attack_ms),
            "hold" => Ok(self.hold_ms),
            "release" => Ok(self.release_ms),
            _ => Err("Unknown parameter name"),
        }
    }
}

// Helper functions
#[inline]
fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[inline]
fn calculate_coefficient(time_ms: f32, sample_rate: f32) -> f32 {
    (-1.0 / (sample_rate * 0.001 * time_ms)).exp()
}
