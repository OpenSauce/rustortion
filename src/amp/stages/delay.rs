use crate::amp::stages::Stage;
use crate::amp::stages::common::calculate_coefficient;

const MAX_DELAY_MS: f32 = 2000.0;
const SMOOTH_TIME_MS: f32 = 50.0;

/// Delay stage for echo and slapback effects.
///
/// Uses a pre-allocated ring buffer (max 2 s) with linear interpolation
/// for fractional delay lengths and one-pole smoothing on the delay time
/// parameter to prevent clicks when the time slider is moved.
pub struct DelayStage {
    delay_ms: f32,
    feedback: f32,
    mix: f32,
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    delay_samples_smoothed: f32,
    delay_samples_target: f32,
    smooth_coeff: f32,
}

impl DelayStage {
    pub fn new(delay_ms: f32, feedback: f32, mix: f32, sample_rate: f32) -> Self {
        let max_samples = (MAX_DELAY_MS * 0.001 * sample_rate) as usize + 1;
        let delay_samples = delay_ms * 0.001 * sample_rate;
        let smooth_coeff = calculate_coefficient(SMOOTH_TIME_MS, sample_rate);

        Self {
            delay_ms,
            feedback,
            mix,
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            delay_samples_smoothed: delay_samples,
            delay_samples_target: delay_samples,
            smooth_coeff,
        }
    }

    fn update_delay_target(&mut self) {
        self.delay_samples_target = self.delay_ms * 0.001 * self.sample_rate;
    }
}

impl Stage for DelayStage {
    fn process(&mut self, input: f32) -> f32 {
        // Smooth delay time to prevent clicks
        self.delay_samples_smoothed = self.smooth_coeff.mul_add(
            self.delay_samples_smoothed,
            (1.0 - self.smooth_coeff) * self.delay_samples_target,
        );

        let buf_len = self.buffer.len();

        // Fractional read position with linear interpolation
        let read_pos = self.write_pos as f32 - self.delay_samples_smoothed + buf_len as f32;
        let read_idx = read_pos as usize % buf_len;
        let frac = read_pos.fract();
        let next_idx = (read_idx + 1) % buf_len;

        let delayed = (1.0 - frac).mul_add(self.buffer[read_idx], frac * self.buffer[next_idx]);

        // Write input + feedback into buffer
        self.buffer[self.write_pos] = self.feedback.mul_add(delayed, input);

        // Advance write position
        self.write_pos = (self.write_pos + 1) % buf_len;

        // Dry/wet mix
        (1.0 - self.mix).mul_add(input, self.mix * delayed)
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "delay_time" => {
                if (0.0..=MAX_DELAY_MS).contains(&value) {
                    self.delay_ms = value;
                    self.update_delay_target();
                    Ok(())
                } else {
                    Err("Delay time must be between 0 ms and 2000 ms")
                }
            }
            "feedback" => {
                if (0.0..=1.0).contains(&value) {
                    self.feedback = value;
                    Ok(())
                } else {
                    Err("Feedback must be between 0.0 and 1.0")
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
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "delay_time" => Ok(self.delay_ms),
            "feedback" => Ok(self.feedback),
            "mix" => Ok(self.mix),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    #[test]
    fn dry_passthrough() {
        let mut delay = DelayStage::new(300.0, 0.0, 0.0, SAMPLE_RATE);
        // With mix = 0, output should equal input
        for i in 0..1000 {
            let input = (i as f32) * 0.001;
            let output = delay.process(input);
            assert!(
                (output - input).abs() < 1e-6,
                "Expected dry passthrough at sample {i}"
            );
        }
    }

    #[test]
    fn wet_only_impulse() {
        let delay_ms = 100.0;
        let delay_samples = (delay_ms * 0.001 * SAMPLE_RATE) as usize;
        let mut delay = DelayStage::new(delay_ms, 0.0, 1.0, SAMPLE_RATE);

        // Warm up the smoother so it converges to target
        for _ in 0..SAMPLE_RATE as usize {
            delay.process(0.0);
        }

        // Send an impulse
        let _ = delay.process(1.0);

        // Process silence until the delay appears
        let mut found = false;
        for i in 1..=delay_samples + 10 {
            let out = delay.process(0.0);
            if i == delay_samples {
                assert!(
                    out > 0.5,
                    "Expected delayed impulse near sample {delay_samples}, got {out}"
                );
                found = true;
            }
        }
        assert!(found, "Did not find delayed impulse");
    }

    #[test]
    fn feedback_decay() {
        let delay_ms = 10.0;
        let delay_samples = (delay_ms * 0.001 * SAMPLE_RATE) as usize;
        let mut delay = DelayStage::new(delay_ms, 0.5, 1.0, SAMPLE_RATE);

        // Warm up smoother
        for _ in 0..SAMPLE_RATE as usize {
            delay.process(0.0);
        }

        // Send impulse
        let _ = delay.process(1.0);

        // Collect peaks at each echo
        let mut peaks = Vec::new();
        let mut count = 0;
        for _ in 0..delay_samples * 5 {
            let out = delay.process(0.0);
            count += 1;
            if count == delay_samples {
                peaks.push(out.abs());
                count = 0;
            }
        }

        // Each successive echo should be quieter
        for window in peaks.windows(2) {
            assert!(
                window[1] < window[0] + 1e-6,
                "Echo should decay: {} >= {}",
                window[1],
                window[0]
            );
        }
    }

    #[test]
    fn parameter_validation() {
        let mut delay = DelayStage::new(300.0, 0.3, 0.3, SAMPLE_RATE);

        assert!(delay.set_parameter("delay_time", -1.0).is_err());
        assert!(delay.set_parameter("delay_time", 2001.0).is_err());
        assert!(delay.set_parameter("delay_time", 500.0).is_ok());

        assert!(delay.set_parameter("feedback", -0.1).is_err());
        assert!(delay.set_parameter("feedback", 1.1).is_err());
        assert!(delay.set_parameter("feedback", 0.5).is_ok());

        assert!(delay.set_parameter("mix", -0.1).is_err());
        assert!(delay.set_parameter("mix", 1.1).is_err());
        assert!(delay.set_parameter("mix", 0.5).is_ok());

        assert!(delay.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn zero_delay_time() {
        let mut delay = DelayStage::new(0.0, 0.0, 1.0, SAMPLE_RATE);
        // Should not panic with zero delay time
        for _ in 0..100 {
            let _ = delay.process(1.0);
        }
    }

    #[test]
    fn get_parameters() {
        let delay = DelayStage::new(300.0, 0.3, 0.5, SAMPLE_RATE);
        assert!((delay.get_parameter("delay_time").unwrap() - 300.0).abs() < 1e-6);
        assert!((delay.get_parameter("feedback").unwrap() - 0.3).abs() < 1e-6);
        assert!((delay.get_parameter("mix").unwrap() - 0.5).abs() < 1e-6);
        assert!(delay.get_parameter("unknown").is_err());
    }
}
