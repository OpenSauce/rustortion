use crate::amp::stages::Stage;
use crate::amp::stages::common::{EnvelopeFollower, calculate_coefficient, db_to_lin};

pub struct CompressorStage {
    attack_ms: f32,  // Attack time in milliseconds
    release_ms: f32, // Release time in milliseconds
    threshold: f32,  // Threshold in linear scale
    ratio: f32,      // Compression ratio (e.g., 4.0 for 4:1)
    makeup: f32,     // Makeup gain in linear scale
    envelope: EnvelopeFollower,
    sample_rate: f32,
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
        Self {
            attack_ms,
            release_ms,
            threshold: db_to_lin(threshold_db),
            ratio,
            makeup: db_to_lin(makeup_db),
            envelope: EnvelopeFollower::from_ms(attack_ms, release_ms, sample_rate),
            sample_rate,
        }
    }

    fn update_attack(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms;
        self.envelope
            .set_attack_coeff(calculate_coefficient(attack_ms, self.sample_rate));
    }

    fn update_release(&mut self, release_ms: f32) {
        self.release_ms = release_ms;
        self.envelope
            .set_release_coeff(calculate_coefficient(release_ms, self.sample_rate));
    }
}

impl Stage for CompressorStage {
    fn process(&mut self, input: f32) -> f32 {
        // Envelope follower (feed abs input, avoid log(0))
        let level_in = input.abs().max(1e-10);
        self.envelope.process(level_in);
        let env = self.envelope.value();

        // Compression gain calculation
        let over_threshold = (env / self.threshold).max(1.0);
        let gain_reduction = if over_threshold > 1.0 {
            over_threshold.powf((1.0 / self.ratio) - 1.0)
        } else {
            1.0
        };

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
            "release" => Ok(self.release_ms),
            "makeup" => Ok(if self.makeup > 1e-10 {
                20.0 * self.makeup.log10()
            } else {
                -200.0
            }),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    /// Helper: threshold -20 dB, ratio 4:1, 0 dB makeup, fast attack/release
    fn make_compressor() -> CompressorStage {
        CompressorStage::new(1.0, 50.0, -20.0, 4.0, 0.0, SR)
    }

    #[test]
    fn test_below_threshold_passthrough() {
        let mut stage = make_compressor(); // threshold = -20 dB ≈ 0.1
        let input = 0.01; // -40 dB, well below threshold
        for _ in 0..2000 {
            stage.process(input);
        }
        let out = stage.process(input);
        assert!(
            (out - input).abs() < 0.005,
            "below threshold should pass through: in={input}, out={out}"
        );
    }

    #[test]
    fn test_above_threshold_compression() {
        let mut stage = make_compressor(); // threshold -20 dB
        let input = 0.5; // well above threshold
        for _ in 0..5000 {
            stage.process(input);
        }
        let out = stage.process(input);
        assert!(
            out.abs() < input.abs(),
            "above threshold should compress: in={input}, out={out}"
        );
    }

    #[test]
    fn test_higher_ratio_more_compression() {
        let mut stage_low = CompressorStage::new(1.0, 50.0, -20.0, 2.0, 0.0, SR);
        let mut stage_high = CompressorStage::new(1.0, 50.0, -20.0, 10.0, 0.0, SR);
        let input = 0.8;
        for _ in 0..5000 {
            stage_low.process(input);
            stage_high.process(input);
        }
        let out_low = stage_low.process(input).abs();
        let out_high = stage_high.process(input).abs();
        assert!(
            out_high < out_low,
            "higher ratio should compress more: 2:1={out_low}, 10:1={out_high}"
        );
    }

    #[test]
    fn test_makeup_gain() {
        let mut no_makeup = CompressorStage::new(1.0, 50.0, -20.0, 4.0, 0.0, SR);
        let mut with_makeup = CompressorStage::new(1.0, 50.0, -20.0, 4.0, 12.0, SR);
        let input = 0.5;
        for _ in 0..5000 {
            no_makeup.process(input);
            with_makeup.process(input);
        }
        let out_no = no_makeup.process(input).abs();
        let out_yes = with_makeup.process(input).abs();
        assert!(
            out_yes > out_no,
            "makeup should boost output: no={out_no}, with={out_yes}"
        );
    }

    #[test]
    fn test_silence_stays_silent() {
        let mut stage = CompressorStage::new(1.0, 50.0, -20.0, 4.0, 12.0, SR);
        for _ in 0..1000 {
            stage.process(0.0);
        }
        let out = stage.process(0.0);
        assert!(
            out.abs() < 1e-6,
            "silence in should produce silence, got {out}"
        );
    }

    #[test]
    fn test_attack_lets_transient_through() {
        let mut stage = CompressorStage::new(100.0, 200.0, -20.0, 10.0, 0.0, SR);
        for _ in 0..2000 {
            stage.process(0.0);
        }
        let first = stage.process(0.8).abs();
        for _ in 0..5000 {
            stage.process(0.8);
        }
        let settled = stage.process(0.8).abs();
        assert!(
            first > settled,
            "slow attack should let transient through: first={first}, settled={settled}"
        );
    }

    #[test]
    fn test_release_recovery() {
        let mut stage = CompressorStage::new(1.0, 100.0, -20.0, 10.0, 0.0, SR);
        for _ in 0..5000 {
            stage.process(0.8);
        }
        let still_compressed = stage.process(0.05).abs();
        for _ in 0..20000 {
            stage.process(0.05);
        }
        let recovered = stage.process(0.05).abs();
        assert!(
            recovered > still_compressed,
            "after release, gain should recover: still_compressed={still_compressed}, recovered={recovered}"
        );
    }

    #[test]
    fn test_bounded_output() {
        let mut stage = CompressorStage::new(1.0, 50.0, -20.0, 4.0, 24.0, SR);
        for i in 0..5000 {
            let input = (i as f32 * 0.1).sin() * 5.0;
            let out = stage.process(input);
            assert!(
                out.is_finite() && out.abs() < 100.0,
                "output must be finite and bounded, got {out}"
            );
        }
    }

    #[test]
    fn test_parameter_validation() {
        let mut stage = make_compressor();
        assert!(stage.set_parameter("threshold", -60.0).is_ok());
        assert!(stage.set_parameter("threshold", 0.0).is_ok());
        assert!(stage.set_parameter("threshold", -61.0).is_err());
        assert!(stage.set_parameter("threshold", 0.1).is_err());
        assert!(stage.set_parameter("ratio", 1.0).is_ok());
        assert!(stage.set_parameter("ratio", 20.0).is_ok());
        assert!(stage.set_parameter("ratio", 0.9).is_err());
        assert!(stage.set_parameter("ratio", 20.1).is_err());
        assert!(stage.set_parameter("attack", 0.1).is_ok());
        assert!(stage.set_parameter("attack", 100.0).is_ok());
        assert!(stage.set_parameter("attack", 0.0).is_err());
        assert!(stage.set_parameter("release", 10.0).is_ok());
        assert!(stage.set_parameter("release", 1000.0).is_ok());
        assert!(stage.set_parameter("release", 9.9).is_err());
        assert!(stage.set_parameter("makeup", -12.0).is_ok());
        assert!(stage.set_parameter("makeup", 24.0).is_ok());
        assert!(stage.set_parameter("makeup", -12.1).is_err());
        assert!(stage.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut stage = make_compressor();
        stage.set_parameter("ratio", 8.0).unwrap();
        assert!((stage.get_parameter("ratio").unwrap() - 8.0).abs() < 1e-6);
        stage.set_parameter("attack", 50.0).unwrap();
        assert!((stage.get_parameter("attack").unwrap() - 50.0).abs() < 1e-6);
        stage.set_parameter("release", 200.0).unwrap();
        assert!((stage.get_parameter("release").unwrap() - 200.0).abs() < 1e-6);
        stage.set_parameter("threshold", -12.0).unwrap();
        assert!((stage.get_parameter("threshold").unwrap() - (-12.0)).abs() < 0.1);
        stage.set_parameter("makeup", 6.0).unwrap();
        assert!((stage.get_parameter("makeup").unwrap() - 6.0).abs() < 0.1);
    }
}
