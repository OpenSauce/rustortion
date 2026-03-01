use crate::amp::stages::Stage;
use crate::amp::stages::common::calculate_coefficient;

const MAX_DELAY_MS: f32 = 2000.0;
const MAX_FEEDBACK: f32 = 0.95;
const SMOOTH_TIME_MS: f32 = 50.0;
const DENORMAL_THRESHOLD: f32 = 1e-20;

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
        let delay_ms = delay_ms.clamp(0.0, MAX_DELAY_MS);
        let feedback = feedback.clamp(0.0, MAX_FEEDBACK);
        let mix = mix.clamp(0.0, 1.0);

        let max_samples = (MAX_DELAY_MS * 0.001 * sample_rate) as usize + 2;
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

        // Clamp to minimum 1 sample so read always gets the previous write, not stale data
        let clamped = self.delay_samples_smoothed.max(1.0);

        // Integer/fractional split — avoids f32 precision loss with large buffers
        let delay_whole = clamped as usize;
        let frac = clamped - delay_whole as f32;

        let read_idx = (self.write_pos + buf_len - delay_whole) % buf_len;
        let prev_idx = (self.write_pos + buf_len - delay_whole - 1) % buf_len;

        // Linear interpolation between the two nearest samples
        let delayed = (1.0 - frac).mul_add(self.buffer[read_idx], frac * self.buffer[prev_idx]);

        // Write input + feedback into buffer, flush denormals
        let write_val = self.feedback.mul_add(delayed, input);
        self.buffer[self.write_pos] = if write_val.abs() < DENORMAL_THRESHOLD {
            0.0
        } else {
            write_val
        };

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
                if (0.0..=MAX_FEEDBACK).contains(&value) {
                    self.feedback = value;
                    Ok(())
                } else {
                    Err("Feedback must be between 0.0 and 0.95")
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
                    out > 0.9,
                    "Expected delayed impulse ~1.0 near sample {delay_samples}, got {out}"
                );
                found = true;
            }
        }
        assert!(found, "Did not find delayed impulse");
    }

    #[test]
    fn wet_only_no_dry_leak() {
        let mut delay = DelayStage::new(100.0, 0.0, 1.0, SAMPLE_RATE);

        // Warm up smoother
        for _ in 0..SAMPLE_RATE as usize {
            delay.process(0.0);
        }

        // With mix = 1.0 and empty buffer, input should not appear in output
        let out = delay.process(1.0);
        assert!(
            out.abs() < 1e-6,
            "At mix=1.0, dry signal should be absent, got {out}"
        );
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

        // Each successive echo must be strictly quieter
        for window in peaks.windows(2) {
            assert!(
                window[1] < window[0],
                "Echo should decay: {} >= {}",
                window[1],
                window[0]
            );
        }
    }

    #[test]
    fn feedback_stays_bounded() {
        // Even at max feedback with sustained input, signal should not grow unbounded
        let mut delay = DelayStage::new(10.0, MAX_FEEDBACK, 1.0, SAMPLE_RATE);

        // Warm up smoother
        for _ in 0..SAMPLE_RATE as usize {
            delay.process(0.0);
        }

        // Feed sustained signal for 2 seconds
        let mut max_out: f32 = 0.0;
        for _ in 0..(SAMPLE_RATE as usize * 2) {
            let out = delay.process(0.5);
            max_out = max_out.max(out.abs());
        }

        // With feedback < 1.0, the geometric series converges:
        // max = input / (1 - feedback) = 0.5 / 0.05 = 10.0
        assert!(max_out < 12.0, "Signal should converge, got max {max_out}");
    }

    #[test]
    fn parameter_validation() {
        let mut delay = DelayStage::new(300.0, 0.3, 0.3, SAMPLE_RATE);

        assert!(delay.set_parameter("delay_time", -1.0).is_err());
        assert!(delay.set_parameter("delay_time", 2001.0).is_err());
        assert!(delay.set_parameter("delay_time", 500.0).is_ok());

        assert!(delay.set_parameter("feedback", -0.1).is_err());
        assert!(delay.set_parameter("feedback", 0.96).is_err());
        assert!(delay.set_parameter("feedback", 0.5).is_ok());

        assert!(delay.set_parameter("mix", -0.1).is_err());
        assert!(delay.set_parameter("mix", 1.1).is_err());
        assert!(delay.set_parameter("mix", 0.5).is_ok());

        assert!(delay.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn constructor_clamps_out_of_range() {
        let delay = DelayStage::new(5000.0, 2.0, 2.0, SAMPLE_RATE);
        assert!((delay.get_parameter("delay_time").unwrap() - MAX_DELAY_MS).abs() < 1e-6);
        assert!((delay.get_parameter("feedback").unwrap() - MAX_FEEDBACK).abs() < 1e-6);
        assert!((delay.get_parameter("mix").unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_delay_time_dry() {
        let mut delay = DelayStage::new(0.0, 0.0, 0.0, SAMPLE_RATE);
        // With mix = 0, should pass through dry signal
        for _ in 0..100 {
            let out = delay.process(1.0);
            assert!(
                (out - 1.0).abs() < 1e-6,
                "Dry passthrough should work at zero delay, got {out}"
            );
        }
    }

    #[test]
    fn zero_delay_time_wet() {
        let mut delay = DelayStage::new(0.0, 0.0, 1.0, SAMPLE_RATE);
        // With mix = 1 and zero delay (clamped to 1 sample), the first
        // output should be silence (buffer is empty), then subsequent
        // outputs should be the previous input (1-sample delay).
        let out = delay.process(1.0);
        assert!(
            out.abs() < 1e-6,
            "First sample should be silence (buffer empty), got {out}"
        );
        let out = delay.process(0.5);
        assert!(
            (out - 1.0).abs() < 1e-6,
            "Second sample should be previous input (1.0), got {out}"
        );
    }

    #[test]
    fn get_parameters() {
        let delay = DelayStage::new(300.0, 0.3, 0.5, SAMPLE_RATE);
        assert!((delay.get_parameter("delay_time").unwrap() - 300.0).abs() < 1e-6);
        assert!((delay.get_parameter("feedback").unwrap() - 0.3).abs() < 1e-6);
        assert!((delay.get_parameter("mix").unwrap() - 0.5).abs() < 1e-6);
        assert!(delay.get_parameter("unknown").is_err());
    }

    #[test]
    fn high_sample_rate_interpolation() {
        // Test at high sample rate to verify no f32 precision issues
        let high_rate = 192_000.0 * 16.0; // 3.072 MHz (max oversampling)
        let delay_ms = 100.5; // fractional to exercise interpolation
        let mut delay = DelayStage::new(delay_ms, 0.0, 1.0, high_rate);

        // Warm up smoother
        let warmup = (high_rate * 2.0) as usize;
        for _ in 0..warmup {
            delay.process(0.0);
        }

        // Send impulse and verify it comes back
        let _ = delay.process(1.0);
        let delay_samples = (delay_ms * 0.001 * high_rate) as usize;
        let mut max_out: f32 = 0.0;
        for _ in 0..delay_samples + 100 {
            let out = delay.process(0.0);
            max_out = max_out.max(out.abs());
        }
        assert!(
            max_out > 0.9,
            "Impulse should survive at high sample rate, got max {max_out}"
        );
    }

    #[test]
    fn parameter_change_mid_processing() {
        let mut delay = DelayStage::new(500.0, 0.0, 1.0, SAMPLE_RATE);

        // Warm up
        for _ in 0..SAMPLE_RATE as usize {
            delay.process(0.0);
        }

        // Change delay time mid-stream and verify no sample exceeds +-1
        delay.set_parameter("delay_time", 100.0).unwrap();
        for _ in 0..SAMPLE_RATE as usize {
            let out = delay.process(0.5);
            assert!(
                out.abs() <= 1.0,
                "Output should stay bounded during parameter change, got {out}"
            );
        }
    }
}
