use crate::amp::stages::Stage;
use crate::amp::stages::clipper::ClipperType;
use crate::amp::stages::common::{DcBlocker, OnePoleLP};

pub struct PreampStage {
    gain: f32,      // 0..10
    bias: f32,      // −1..+1
    bias_comp: f32, // cosh²(bias), cached for RT performance
    clipper_type: ClipperType,
    interstage_lp: OnePoleLP,
    dc_blocker: DcBlocker,
}

impl PreampStage {
    pub fn new(gain: f32, bias: f32, clipper: ClipperType, sample_rate: f32) -> Self {
        let bias = bias.clamp(-1.0, 1.0);
        Self {
            gain,
            bias,
            bias_comp: bias.cosh().powi(2).min(4.0),
            clipper_type: clipper,
            interstage_lp: OnePoleLP::new(10_000.0, sample_rate),
            dc_blocker: DcBlocker::new(15.0, sample_rate),
        }
    }
}

impl Stage for PreampStage {
    fn process(&mut self, input: f32) -> f32 {
        const DRIVE_MIN: f32 = 1.0;
        const DRIVE_SCALE: f32 = 1.8;
        const CLIPPER_SCALE: f32 = 0.3;

        let drive = self.gain.mul_add(DRIVE_SCALE, DRIVE_MIN);

        // --- Initial asymmetric soft clip with DC compensation ---
        // Instead of adding DC to the input, shift the tanh curve and recenter:
        let pre = (drive.mul_add(input, self.bias).tanh() - self.bias.tanh()) * self.bias_comp;

        // Inter-stage lowpass: models plate load capacitance rolling off upper
        // harmonics before they reach the next nonlinearity. Without this,
        // cascaded waveshapers re-distort the full harmonic spectrum, producing fizz.
        let filtered = self.interstage_lp.process(pre);

        // Main clipper expects roughly zero-centered signal; keep threshold tied to gain
        let clipped = self
            .clipper_type
            .process(filtered, self.gain.mul_add(CLIPPER_SCALE, 1.0));

        // Remove any residual DC so next stage gets a clean, centered signal
        self.dc_blocker.process(clipped)
    }

    fn set_parameter(&mut self, p: &str, v: f32) -> Result<(), &'static str> {
        match p {
            "gain" => {
                if (0.0..=10.0).contains(&v) {
                    self.gain = v;
                    Ok(())
                } else {
                    Err("Gain 0-10")
                }
            }
            "bias" => {
                if (-1.0..=1.0).contains(&v) {
                    self.bias = v;
                    self.bias_comp = v.cosh().powi(2).min(4.0);
                    Ok(())
                } else {
                    Err("Bias −1-1")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, p: &str) -> Result<f32, &'static str> {
        match p {
            "gain" => Ok(self.gain),
            "bias" => Ok(self.bias),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn make_preamp(gain: f32, bias: f32) -> PreampStage {
        PreampStage::new(gain, bias, ClipperType::Soft, SR)
    }

    #[test]
    fn test_zero_input_silence() {
        let mut stage = make_preamp(5.0, 0.0);
        for _ in 0..1000 {
            stage.process(0.0);
        }
        let out = stage.process(0.0);
        assert!(
            out.abs() < 1e-6,
            "zero input should produce silence, got {out}"
        );
    }

    #[test]
    fn test_bounded_output() {
        for gain in [0.0, 5.0, 10.0] {
            for clipper in [ClipperType::Soft, ClipperType::Hard, ClipperType::Triode] {
                let mut stage = PreampStage::new(gain, 0.0, clipper, SR);
                for i in 0..2000 {
                    let input = (i as f32 / 100.0).sin() * 5.0;
                    let out = stage.process(input);
                    assert!(
                        out.is_finite() && out.abs() < 10.0,
                        "output must be finite and bounded, got {out} (gain={gain}, clipper={clipper:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_higher_gain_more_distortion() {
        // Measure harmonic distortion energy: RMS of (output - scaled_input).
        // Higher gain should produce more harmonic content beyond the fundamental.
        fn distortion_energy(gain: f32) -> f32 {
            let mut stage = make_preamp(gain, 0.0);
            // Warm up
            for i in 0..500 {
                stage.process((i as f32 * 0.1).sin() * 0.3);
            }
            let mut sum_in2 = 0.0_f32;
            let mut sum_diff2 = 0.0_f32;
            let n = 4000;
            for i in 0..n {
                let input = (i as f32 * 0.1).sin() * 0.3;
                let out = stage.process(input);
                sum_in2 += input * input;
                sum_diff2 += (out - input) * (out - input);
            }
            let in_rms = (sum_in2 / n as f32).sqrt();
            if in_rms < 1e-10 {
                return 0.0;
            }
            // Normalized distortion: RMS(out - in) / RMS(in)
            (sum_diff2 / n as f32).sqrt() / in_rms
        }
        let low_gain_dist = distortion_energy(1.0);
        let high_gain_dist = distortion_energy(10.0);
        assert!(
            high_gain_dist > low_gain_dist,
            "high gain should produce more distortion: low={low_gain_dist}, high={high_gain_dist}"
        );
    }

    #[test]
    fn test_bias_asymmetry() {
        let mut pos_bias = make_preamp(5.0, 0.8);
        let mut neg_bias = make_preamp(5.0, -0.8);

        for i in 0..500 {
            let x = (i as f32 * 0.05).sin() * 0.5;
            pos_bias.process(x);
            neg_bias.process(x);
        }

        let mut sum_diff = 0.0_f32;
        for i in 0..2000 {
            let x = (i as f32 * 0.05).sin() * 0.5;
            let a = pos_bias.process(x);
            let b = neg_bias.process(x);
            sum_diff += (a - b).abs();
        }
        assert!(
            sum_diff > 1.0,
            "different bias values should produce different outputs, diff={sum_diff}"
        );
    }

    #[test]
    fn test_dc_rejection() {
        let mut stage = make_preamp(3.0, 0.0);
        for _ in 0..48000 {
            stage.process(0.5);
        }
        let mut avg = 0.0_f32;
        let n = 4096;
        for _ in 0..n {
            avg += stage.process(0.5);
        }
        avg /= n as f32;
        assert!(
            avg.abs() < 0.1,
            "DC blocker should remove DC offset, avg={avg}"
        );
    }

    #[test]
    fn test_parameter_validation() {
        let mut stage = make_preamp(5.0, 0.0);
        assert!(stage.set_parameter("gain", 0.0).is_ok());
        assert!(stage.set_parameter("gain", 10.0).is_ok());
        assert!(stage.set_parameter("gain", -0.1).is_err());
        assert!(stage.set_parameter("gain", 10.1).is_err());
        assert!(stage.set_parameter("bias", -1.0).is_ok());
        assert!(stage.set_parameter("bias", 1.0).is_ok());
        assert!(stage.set_parameter("bias", -1.1).is_err());
        assert!(stage.set_parameter("bias", 1.1).is_err());
        assert!(stage.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut stage = make_preamp(5.0, 0.0);
        stage.set_parameter("gain", 7.5).unwrap();
        assert!((stage.get_parameter("gain").unwrap() - 7.5).abs() < 1e-6);
        stage.set_parameter("bias", -0.3).unwrap();
        assert!((stage.get_parameter("bias").unwrap() - (-0.3)).abs() < 1e-6);
        assert!(stage.get_parameter("unknown").is_err());
    }

    #[test]
    fn test_sample_rate_consistency() {
        for sr in [44100.0_f32, 48000.0, 96000.0] {
            let mut stage = PreampStage::new(5.0, 0.0, ClipperType::Soft, sr);
            for i in 0..((sr * 0.05) as usize) {
                stage.process((i as f32 * 0.1).sin() * 0.5);
            }
            let out = stage.process(0.5);
            assert!(out.is_finite(), "output not finite at sr={sr}");
            assert!(out.abs() < 5.0, "output unbounded at sr={sr}, got {out}");
        }
    }

    #[test]
    fn test_bias_level_consistency() {
        // TUBE-5: RMS output level should be within ±1.5 dB across bias range
        fn measure_rms(bias: f32) -> f32 {
            let mut stage = PreampStage::new(5.0, bias, ClipperType::Soft, SR);
            // Long warmup for DC blocker to settle (15 Hz HP needs time)
            for i in 0..48000 {
                stage.process((i as f32 * 0.05).sin() * 0.3);
            }
            let mut sum2 = 0.0_f32;
            let n = 8000;
            for i in 0..n {
                let input = (i as f32 * 0.05).sin() * 0.3;
                let out = stage.process(input);
                sum2 += out * out;
            }
            (sum2 / n as f32).sqrt()
        }

        let rms_zero = measure_rms(0.0);
        for bias in [-1.0, -0.5, 0.5, 1.0] {
            let rms = measure_rms(bias);
            let db_diff = 20.0 * (rms / rms_zero).log10();
            assert!(
                db_diff.abs() < 1.5,
                "RMS at bias={bias} differs by {db_diff:.1} dB from bias=0 (rms={rms:.4}, ref={rms_zero:.4})"
            );
        }
    }

    #[test]
    fn test_zero_input_silence_with_bias() {
        // TUBE-5: Zero input must produce silence even with bias compensation
        for bias in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let mut stage = PreampStage::new(5.0, bias, ClipperType::Soft, SR);
            // Extra warmup — bias_comp amplifies DC before blocker
            for _ in 0..48000 {
                stage.process(0.0);
            }
            let out = stage.process(0.0);
            assert!(
                out.abs() < 1e-4,
                "zero input should produce silence at bias={bias}, got {out}"
            );
        }
    }

    #[test]
    fn test_asymmetric_clipper_with_bias_bounded() {
        // Combined regression: TUBE-3 + TUBE-5 interaction
        for bias in [-1.0, -0.5, 0.5, 1.0] {
            let mut stage = PreampStage::new(10.0, bias, ClipperType::Asymmetric, SR);
            for i in 0..48000 {
                let input = (i as f32 * 0.1).sin() * 5.0;
                let out = stage.process(input);
                assert!(
                    out.is_finite() && out.abs() < 10.0,
                    "output must be finite and bounded, got {out} (bias={bias})"
                );
            }
        }
    }
}
