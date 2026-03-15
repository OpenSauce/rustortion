use crate::amp::stages::Stage;
use crate::amp::stages::common::{EnvelopeFollower, calculate_coefficient, db_to_lin};

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
    envelope: EnvelopeFollower, // Input level envelope
    gate_state: f32,            // Current gate state (0 = closed, 1 = open)
    hold_counter: usize,        // Sample counter for hold time
    sample_rate: f32,

    // Gate smoothing coefficients
    attack_coeff: f32,
    release_coeff: f32,
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

        let attack_coeff = calculate_coefficient(attack_ms, sample_rate);
        let release_coeff = calculate_coefficient(release_ms, sample_rate);

        // Envelope follower: fast attack (0.1ms), moderate release (10ms)
        let envelope = EnvelopeFollower::from_ms(0.1, 10.0, sample_rate);

        Self {
            threshold,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            envelope,
            gate_state: 0.0,
            hold_counter: 0,
            sample_rate,
            attack_coeff,
            release_coeff,
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
        self.envelope.process(input);
        let env = self.envelope.value();

        // Step 2: Determine if gate should be open or closed
        let should_open = env > self.threshold;

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
            self.gate_state = self
                .attack_coeff
                .mul_add(self.gate_state, (1.0 - self.attack_coeff) * target_state);
        } else {
            // Closing (release)
            self.gate_state = self
                .release_coeff
                .mul_add(self.gate_state, (1.0 - self.release_coeff) * target_state);
        }

        // Step 5: Apply gating with ratio
        let reduction = if self.gate_state < 0.999 {
            let closed_gain = 1.0 / self.ratio;
            (1.0 - closed_gain).mul_add(self.gate_state, closed_gain)
        } else {
            1.0
        };

        input * reduction
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
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "threshold" => Ok(if self.threshold > 1e-10 {
                20.0 * self.threshold.log10()
            } else {
                -200.0
            }),
            "ratio" => Ok(self.ratio),
            "attack" => Ok(self.attack_ms),
            "hold" => Ok(self.hold_ms),
            "release" => Ok(self.release_ms),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn make_gate() -> NoiseGateStage {
        // threshold -30 dB, ratio 10:1, 1ms attack, 50ms hold, 50ms release
        NoiseGateStage::new(-30.0, 10.0, 1.0, 50.0, 50.0, SR)
    }

    #[test]
    fn test_loud_signal_passes() {
        let mut gate = make_gate(); // threshold -30 dB ≈ 0.0316
        let input = 0.5; // well above threshold
        for _ in 0..2000 {
            gate.process(input);
        }
        let out = gate.process(input);
        assert!(
            (out - input).abs() < 0.05,
            "loud signal should pass through: in={input}, out={out}"
        );
    }

    #[test]
    fn test_quiet_signal_attenuated() {
        let mut gate = make_gate(); // threshold -30 dB
        let input = 0.001; // well below threshold (-60 dB)
        for _ in 0..10000 {
            gate.process(input);
        }
        let out = gate.process(input);
        assert!(
            out.abs() < input.abs() * 0.5,
            "quiet signal should be attenuated: in={input}, out={out}"
        );
    }

    #[test]
    fn test_hold_time() {
        let hold_ms = 100.0;
        let release_ms = 50.0;
        let mut gate_with_hold = NoiseGateStage::new(-30.0, 10.0, 1.0, hold_ms, release_ms, SR);
        let mut gate_no_hold = NoiseGateStage::new(-30.0, 10.0, 1.0, 0.0, release_ms, SR);
        let probe = 0.02; // below threshold but nonzero so we can measure gate state

        // Open both gates
        for _ in 0..2000 {
            gate_with_hold.process(0.5);
            gate_no_hold.process(0.5);
        }
        // Feed below-threshold signal and measure after half the hold period
        let half_hold = ((hold_ms * 0.001 * SR) / 2.0) as usize;
        for _ in 0..half_hold {
            gate_with_hold.process(probe);
            gate_no_hold.process(probe);
        }
        let out_with_hold = gate_with_hold.process(probe).abs();
        let out_no_hold = gate_no_hold.process(probe).abs();
        // Gate with hold should pass more signal (still open) vs no-hold (already closing)
        assert!(
            out_with_hold > out_no_hold,
            "hold should keep gate open longer: with_hold={out_with_hold}, no_hold={out_no_hold}"
        );
    }

    #[test]
    fn test_ratio_controls_attenuation() {
        let mut gate_low = NoiseGateStage::new(-30.0, 2.0, 1.0, 0.0, 50.0, SR);
        let mut gate_high = NoiseGateStage::new(-30.0, 100.0, 1.0, 0.0, 50.0, SR);
        let input = 0.001; // below threshold
        for _ in 0..10000 {
            gate_low.process(input);
            gate_high.process(input);
        }
        let out_low = gate_low.process(input).abs();
        let out_high = gate_high.process(input).abs();
        assert!(
            out_high < out_low,
            "higher ratio should attenuate more: 2:1={out_low}, 100:1={out_high}"
        );
    }

    #[test]
    fn test_zero_input_attenuated() {
        let mut gate = make_gate();
        for _ in 0..5000 {
            gate.process(0.0);
        }
        let out = gate.process(0.0);
        assert!(
            out.abs() < 1e-10,
            "zero input should produce zero output, got {out}"
        );
    }

    #[test]
    fn test_smooth_transitions() {
        // Gate closing should be gradual (release smoothing), not instant
        let mut gate = NoiseGateStage::new(-30.0, 100.0, 1.0, 0.0, 100.0, SR);
        let probe = 0.02; // below threshold, nonzero to observe gate gain
        // Open the gate
        for _ in 0..2000 {
            gate.process(0.5);
        }
        // Now feed below-threshold signal — gate should close gradually
        let first = gate.process(probe).abs();
        // Skip some samples into the release
        for _ in 0..500 {
            gate.process(probe);
        }
        let mid = gate.process(probe).abs();
        // Skip more
        for _ in 0..5000 {
            gate.process(probe);
        }
        let late = gate.process(probe).abs();
        // First should pass more than mid, mid more than late (gradual closing)
        assert!(
            first >= mid && mid >= late,
            "gate should close gradually: first={first}, mid={mid}, late={late}"
        );
        // And the range should be meaningful — not all the same
        assert!(
            first > late * 1.1,
            "gate should actually attenuate over time: first={first}, late={late}"
        );
    }

    #[test]
    fn test_bounded_output() {
        let mut gate = make_gate();
        for i in 0..5000 {
            let input = (i as f32 * 0.1).sin() * 5.0;
            let out = gate.process(input);
            assert!(
                out.is_finite() && out.abs() <= input.abs() + 0.01,
                "gate should never amplify: in={input}, out={out}"
            );
        }
    }

    #[test]
    fn test_parameter_validation() {
        let mut gate = make_gate();
        assert!(gate.set_parameter("threshold", -80.0).is_ok());
        assert!(gate.set_parameter("threshold", 0.0).is_ok());
        assert!(gate.set_parameter("threshold", -80.1).is_err());
        assert!(gate.set_parameter("threshold", 0.1).is_err());
        assert!(gate.set_parameter("ratio", 1.0).is_ok());
        assert!(gate.set_parameter("ratio", 100.0).is_ok());
        assert!(gate.set_parameter("ratio", 0.9).is_err());
        assert!(gate.set_parameter("ratio", 100.1).is_err());
        assert!(gate.set_parameter("attack", 0.1).is_ok());
        assert!(gate.set_parameter("attack", 100.0).is_ok());
        assert!(gate.set_parameter("attack", 0.0).is_err());
        assert!(gate.set_parameter("hold", 0.0).is_ok());
        assert!(gate.set_parameter("hold", 500.0).is_ok());
        assert!(gate.set_parameter("hold", -0.1).is_err());
        assert!(gate.set_parameter("release", 1.0).is_ok());
        assert!(gate.set_parameter("release", 1000.0).is_ok());
        assert!(gate.set_parameter("release", 0.9).is_err());
        assert!(gate.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut gate = make_gate();
        gate.set_parameter("ratio", 50.0).unwrap();
        assert!((gate.get_parameter("ratio").unwrap() - 50.0).abs() < 1e-6);
        gate.set_parameter("attack", 10.0).unwrap();
        assert!((gate.get_parameter("attack").unwrap() - 10.0).abs() < 1e-6);
        gate.set_parameter("hold", 200.0).unwrap();
        assert!((gate.get_parameter("hold").unwrap() - 200.0).abs() < 1e-6);
        gate.set_parameter("release", 500.0).unwrap();
        assert!((gate.get_parameter("release").unwrap() - 500.0).abs() < 1e-6);
        // threshold is stored linear, returned as dB
        gate.set_parameter("threshold", -40.0).unwrap();
        assert!((gate.get_parameter("threshold").unwrap() - (-40.0)).abs() < 0.5);
        assert!(gate.get_parameter("unknown").is_err());
    }

    #[test]
    fn threshold_zero_returns_finite_floor() {
        let mut gate = make_gate();
        gate.threshold = 0.0;
        let db = gate.get_parameter("threshold").unwrap();
        assert!(db.is_finite());
        assert_eq!(db, -200.0);
    }
}
