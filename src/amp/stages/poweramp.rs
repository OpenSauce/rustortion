use crate::amp::stages::Stage;
use crate::amp::stages::common::{DcBlocker, EnvelopeFollower, calculate_coefficient};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PowerAmpType {
    ClassA,
    ClassAB,
    ClassB,
}

impl std::fmt::Display for PowerAmpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClassA => write!(f, "{}", crate::tr!(poweramp_class_a)),
            Self::ClassAB => write!(f, "{}", crate::tr!(poweramp_class_ab)),
            Self::ClassB => write!(f, "{}", crate::tr!(poweramp_class_b)),
        }
    }
}

pub struct PowerAmpStage {
    drive: f32,
    amp_type: PowerAmpType,
    sag: f32,
    sag_release: f32,
    sag_envelope: EnvelopeFollower,
    dc_blocker: DcBlocker,
    sample_rate: f32,
}

/// Sag release range in milliseconds: tight (40ms) to spongy (200ms).
const SAG_RELEASE_MIN_MS: f32 = 40.0;
const SAG_RELEASE_MAX_MS: f32 = 200.0;

impl PowerAmpStage {
    pub fn new(
        drive: f32,
        amp_type: PowerAmpType,
        sag: f32,
        sag_release_ms: f32,
        sample_rate: f32,
    ) -> Self {
        let sag_release_ms = sag_release_ms.clamp(SAG_RELEASE_MIN_MS, SAG_RELEASE_MAX_MS);
        Self {
            drive: drive.clamp(0.0, 1.0),
            amp_type,
            sag: sag.clamp(0.0, 1.0),
            sag_release: sag_release_ms,
            sag_envelope: EnvelopeFollower::from_ms(10.0, sag_release_ms, sample_rate),
            dc_blocker: DcBlocker::new(10.0, sample_rate),
            sample_rate,
        }
    }
}

impl Stage for PowerAmpStage {
    fn process(&mut self, input: f32) -> f32 {
        let driven = input * self.drive.mul_add(3.0, 1.0);

        self.sag_envelope.process(driven);

        if self.sag_envelope.value().abs() < 1e-20 {
            self.sag_envelope.reset();
        }

        let ceiling = (self.sag * self.sag_envelope.value())
            .mul_add(-0.5, 1.0)
            .max(0.1);

        let clipped = match self.amp_type {
            PowerAmpType::ClassA => {
                if driven >= 0.0 {
                    (driven / ceiling).tanh() * ceiling
                } else {
                    (driven * 0.8 / ceiling).tanh() * ceiling
                }
            }
            PowerAmpType::ClassAB => {
                let dz: f32 = 0.1;
                let x2 = driven * driven;
                let crossover = driven * x2 / (dz.mul_add(dz, x2));
                (crossover / ceiling).tanh() * ceiling
            }
            PowerAmpType::ClassB => {
                let dz: f32 = 0.25;
                let x2 = driven * driven;
                let crossover = driven * x2 / (dz.mul_add(dz, x2));
                (crossover / ceiling).tanh() * ceiling
            }
        };

        self.dc_blocker.process(clipped)
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "drive" => {
                if (0.0..=1.0).contains(&value) {
                    self.drive = value;
                    Ok(())
                } else {
                    Err("Drive must be between 0.0 and 1.0")
                }
            }
            "sag" => {
                if (0.0..=1.0).contains(&value) {
                    self.sag = value;
                    Ok(())
                } else {
                    Err("Sag must be between 0.0 and 1.0")
                }
            }
            "sag_release" => {
                if (SAG_RELEASE_MIN_MS..=SAG_RELEASE_MAX_MS).contains(&value) {
                    self.sag_release = value;
                    self.sag_envelope
                        .set_release_coeff(calculate_coefficient(value, self.sample_rate));
                    Ok(())
                } else {
                    Err("Sag release must be between 40.0 and 200.0 ms")
                }
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "drive" => Ok(self.drive),
            "sag" => Ok(self.sag),
            "sag_release" => Ok(self.sag_release),
            _ => Err("Unknown parameter name"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48000.0;

    fn make_stage(
        amp_type: PowerAmpType,
        drive: f32,
        sag: f32,
        sag_release_ms: f32,
    ) -> PowerAmpStage {
        PowerAmpStage::new(drive, amp_type, sag, sag_release_ms, SAMPLE_RATE)
    }

    #[test]
    fn test_class_ab_symmetric() {
        let mut stage_pos = make_stage(PowerAmpType::ClassAB, 0.5, 0.0, 120.0);
        let mut stage_neg = make_stage(PowerAmpType::ClassAB, 0.5, 0.0, 120.0);
        for i in 1..=20 {
            let x = i as f32 * 0.1;
            let pos = stage_pos.process(x);
            let neg = stage_neg.process(-x);
            assert!(
                (pos + neg).abs() < 1e-6,
                "ClassAB not symmetric at x={x}: pos={pos}, neg={neg}"
            );
        }
    }

    #[test]
    fn test_class_b_symmetric() {
        let mut stage_pos = make_stage(PowerAmpType::ClassB, 0.5, 0.0, 120.0);
        let mut stage_neg = make_stage(PowerAmpType::ClassB, 0.5, 0.0, 120.0);
        for i in 1..=20 {
            let x = i as f32 * 0.1;
            let pos = stage_pos.process(x);
            let neg = stage_neg.process(-x);
            assert!(
                (pos + neg).abs() < 1e-6,
                "ClassB not symmetric at x={x}: pos={pos}, neg={neg}"
            );
        }
    }

    #[test]
    fn test_class_b_more_crossover_than_ab() {
        let mut ab = make_stage(PowerAmpType::ClassAB, 0.5, 0.0, 120.0);
        let mut b = make_stage(PowerAmpType::ClassB, 0.5, 0.0, 120.0);
        let small_input = 0.05;
        let ab_out = ab.process(small_input).abs();
        let b_out = b.process(small_input).abs();
        assert!(
            b_out < ab_out,
            "ClassB should have more crossover attenuation: AB={ab_out}, B={b_out}"
        );
    }

    #[test]
    fn test_class_a_asymmetric() {
        let mut stage_pos = make_stage(PowerAmpType::ClassA, 0.5, 0.0, 120.0);
        let mut stage_neg = make_stage(PowerAmpType::ClassA, 0.5, 0.0, 120.0);
        let pos = stage_pos.process(0.5);
        let neg = stage_neg.process(-0.5);
        assert!(
            (pos.abs() - neg.abs()).abs() > 0.01,
            "ClassA should be asymmetric: pos={pos}, neg={neg}"
        );
    }

    #[test]
    fn test_sag_zero_no_effect() {
        // sag=0 stage vs sag=1 stage: after warmup, sag=0 should produce higher output
        // because sag=0 means the ceiling stays at 1.0 and is not reduced by the envelope
        let mut no_sag = make_stage(PowerAmpType::ClassAB, 0.5, 0.0, 120.0);
        let mut with_sag = make_stage(PowerAmpType::ClassAB, 0.5, 1.0, 120.0);

        // Warm up both envelopes
        for _ in 0..2000 {
            no_sag.process(0.8);
            with_sag.process(0.8);
        }

        let out_no_sag = no_sag.process(0.8);
        let out_with_sag = with_sag.process(0.8);

        // sag=0 output should be larger (no ceiling reduction)
        assert!(
            out_no_sag.abs() > out_with_sag.abs(),
            "sag=0 should produce higher output than sag=1: no_sag={out_no_sag}, with_sag={out_with_sag}"
        );
    }

    #[test]
    fn test_sag_increases_distortion() {
        let mut stage = make_stage(PowerAmpType::ClassAB, 0.8, 1.0, 120.0);
        for _ in 0..2000 {
            stage.process(0.9);
        }
        let saggy_output = stage.process(0.9);

        let mut clean_stage = make_stage(PowerAmpType::ClassAB, 0.8, 0.0, 120.0);
        for _ in 0..2000 {
            clean_stage.process(0.9);
        }
        let clean_output = clean_stage.process(0.9);

        assert!(
            saggy_output.abs() < clean_output.abs(),
            "sag should reduce output level (ceiling drops): saggy={saggy_output}, clean={clean_output}"
        );
    }

    #[test]
    fn test_ceiling_floor_clamp() {
        let mut stage = make_stage(PowerAmpType::ClassAB, 1.0, 1.0, 120.0);
        for _ in 0..10000 {
            let out = stage.process(5.0);
            assert!(out.is_finite(), "output must be finite");
            assert!(out.abs() < 2.0, "output must be bounded, got {out}");
        }
    }

    #[test]
    fn test_sag_release_parameter() {
        let mut stage = make_stage(PowerAmpType::ClassAB, 0.5, 0.5, 40.0);
        assert!((stage.get_parameter("sag_release").unwrap() - 40.0).abs() < 1e-6);
        stage.set_parameter("sag_release", 200.0).unwrap();
        assert!((stage.get_parameter("sag_release").unwrap() - 200.0).abs() < 1e-6);
        // Out of range should fail
        assert!(stage.set_parameter("sag_release", 201.0).is_err());
        assert!(stage.set_parameter("sag_release", 39.0).is_err());
    }

    #[test]
    fn test_denormal_protection() {
        let mut stage = make_stage(PowerAmpType::ClassAB, 0.5, 0.5, 120.0);
        for _ in 0..1000 {
            stage.process(1.0);
        }
        for _ in 0..100_000 {
            stage.process(0.0);
        }
        let envelope = stage.sag_envelope.value();
        assert!(
            envelope == 0.0 || envelope.abs() > 1e-20,
            "envelope should be zero or normal, got {envelope}"
        );
    }

    #[test]
    fn test_sample_rate_consistency() {
        for sr in [44100.0_f32, 48000.0, 96000.0] {
            let mut stage = PowerAmpStage::new(0.5, PowerAmpType::ClassAB, 0.8, 120.0, sr);
            for _ in 0..((sr * 0.1) as usize) {
                stage.process(0.9);
            }
            let out = stage.process(0.9);
            assert!(out.is_finite(), "output not finite at sr={sr}");
            assert!(
                out.abs() < 1.0,
                "output should be bounded at sr={sr}, got {out}"
            );
        }
    }

    #[test]
    fn test_sag_release_recovery_speed() {
        let mut tight = make_stage(PowerAmpType::ClassAB, 0.5, 1.0, 40.0);
        let mut spongy = make_stage(PowerAmpType::ClassAB, 0.5, 1.0, 200.0);
        for _ in 0..5000 {
            tight.process(1.0);
            spongy.process(1.0);
        }
        for _ in 0..5000 {
            tight.process(0.0);
            spongy.process(0.0);
        }
        let tight_env = tight.sag_envelope.value();
        let spongy_env = spongy.sag_envelope.value();
        assert!(
            tight_env < spongy_env,
            "tight should recover faster: tight={tight_env}, spongy={spongy_env}"
        );
    }

    #[test]
    fn test_class_a_dc_blocker() {
        let mut stage = make_stage(PowerAmpType::ClassA, 0.8, 0.0, 120.0);
        // Warm up: let DC blocker settle (10 Hz cutoff needs many samples)
        for _ in 0..48000 {
            stage.process(0.5);
        }
        let n = 10000;
        let mut sum = 0.0_f64;
        for _ in 0..n {
            sum += f64::from(stage.process(0.5));
        }
        let avg = sum / n as f64;
        assert!(
            avg.abs() < 0.05,
            "DC blocker should remove offset, average was {avg}"
        );
    }
}
