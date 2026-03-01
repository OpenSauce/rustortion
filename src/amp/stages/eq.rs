use std::f64::consts::PI;

use crate::amp::stages::Stage;

pub const NUM_BANDS: usize = 16;
pub const BAND_FREQS: [f64; NUM_BANDS] = [
    25.0, 40.0, 63.0, 100.0, 160.0, 250.0, 400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0,
    10000.0, 16000.0, 20000.0,
];
pub const MIN_GAIN_DB: f32 = -12.0;
pub const MAX_GAIN_DB: f32 = 12.0;
const DENORMAL_THRESHOLD: f64 = 1e-20;

/// Bandwidth in octaves: 10 octaves / 16 bands
const BANDWIDTH: f64 = 10.0 / NUM_BANDS as f64;

/// Direct Form 1 biquad filter for peaking EQ.
///
/// Uses f64 internally for coefficient computation and state to avoid
/// numerical instability at low frequencies (e.g. 25 Hz at high sample
/// rates), where f32 poles sit too close to the unit circle.
#[derive(Clone)]
struct Biquad {
    // Normalized coefficients (f64 for precision at low freq / high SR)
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    // State variables (f64 to match coefficient precision)
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    /// Create a unity passthrough biquad.
    const fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Set coefficients for a peaking EQ band using Audio EQ Cookbook formulas.
    ///
    /// Uses the BW-in-octaves alpha formula so that every band maintains
    /// constant-octave bandwidth regardless of its position relative to
    /// the sample rate:
    ///   `alpha = sin(w0) * sinh(ln(2)/2 * BW * w0/sin(w0))`
    fn set_peaking_eq(&mut self, freq: f64, gain_db: f64, bw: f64, sample_rate: f64) {
        let was_unity = self.b1 == 0.0 && self.a1 == 0.0;

        if gain_db.abs() < 1e-6 {
            // Unity passthrough — skip computation.
            // Reset state: values from the old filter shape are meaningless
            // to a passthrough and would cause a transient on the next
            // non-zero coefficient update.
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
            self.x1 = 0.0;
            self.x2 = 0.0;
            self.y1 = 0.0;
            self.y2 = 0.0;
            return;
        }

        // Reset state when transitioning from unity passthrough — the stored
        // state (all zeros from passthrough) is trivially compatible, but when
        // transitioning from a *different* active filter shape through unity
        // and back, we want a clean slate. For gain-to-gain changes, DF1
        // state remains meaningful and transitions smoothly by design.
        if was_unity {
            self.x1 = 0.0;
            self.x2 = 0.0;
            self.y1 = 0.0;
            self.y2 = 0.0;
        }

        // Nyquist guard
        let freq = freq.min(sample_rate * 0.499);

        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();

        // Audio EQ Cookbook: alpha from BW in octaves
        // alpha = sin(w0) * sinh(ln(2)/2 * BW * w0/sin(w0))
        let alpha = sin_w0 * (f64::ln(2.0) / 2.0 * bw * w0 / sin_w0).sinh();

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        // Normalize by a0
        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    /// Process a single sample through the DF1 difference equation.
    #[inline]
    fn process(&mut self, input: f64) -> f64 {
        let y = self
            .b0
            .mul_add(input, self.b1.mul_add(self.x1, self.b2 * self.x2))
            - self.a1.mul_add(self.y1, self.a2 * self.y2);

        // Flush denormals
        let y = if y.abs() < DENORMAL_THRESHOLD { 0.0 } else { y };

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = y;

        y
    }
}

/// 16-band graphic EQ stage using cascaded biquad peaking filters.
pub struct EqStage {
    biquads: [Biquad; NUM_BANDS],
    gains_db: [f32; NUM_BANDS],
    sample_rate: f64,
}

impl EqStage {
    pub fn new(gains_db: [f32; NUM_BANDS], sample_rate: f32) -> Self {
        let sr = f64::from(sample_rate);
        let mut biquads = std::array::from_fn(|_| Biquad::new());

        for (i, biquad) in biquads.iter_mut().enumerate() {
            let gain = gains_db[i].clamp(MIN_GAIN_DB, MAX_GAIN_DB);
            biquad.set_peaking_eq(BAND_FREQS[i], f64::from(gain), BANDWIDTH, sr);
        }

        Self {
            biquads,
            gains_db: gains_db.map(|g| g.clamp(MIN_GAIN_DB, MAX_GAIN_DB)),
            sample_rate: sr,
        }
    }

    #[cfg(test)]
    fn band_param_name(index: usize) -> String {
        format!("band_{index}")
    }

    fn parse_band_index(name: &str) -> Option<usize> {
        name.strip_prefix("band_")?.parse().ok()
    }
}

impl Stage for EqStage {
    fn process(&mut self, input: f32) -> f32 {
        let mut sample = f64::from(input);
        for biquad in &mut self.biquads {
            sample = biquad.process(sample);
        }
        #[allow(clippy::cast_possible_truncation)]
        let out = sample as f32;
        out
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        let idx =
            Self::parse_band_index(name).ok_or("Unknown parameter (expected band_0..=band_15)")?;
        if idx >= NUM_BANDS {
            return Err("Band index out of range (0..=15)");
        }
        if !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(&value) {
            return Err("Gain must be between -12 dB and +12 dB");
        }
        self.gains_db[idx] = value;
        self.biquads[idx].set_peaking_eq(
            BAND_FREQS[idx],
            f64::from(value),
            BANDWIDTH,
            self.sample_rate,
        );
        Ok(())
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        let idx =
            Self::parse_band_index(name).ok_or("Unknown parameter (expected band_0..=band_15)")?;
        if idx >= NUM_BANDS {
            return Err("Band index out of range (0..=15)");
        }
        Ok(self.gains_db[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    fn flat_gains() -> [f32; NUM_BANDS] {
        [0.0; NUM_BANDS]
    }

    #[test]
    fn flat_passthrough() {
        let mut eq = EqStage::new(flat_gains(), SAMPLE_RATE);

        // Feed a signal through the flat EQ — should pass through unchanged
        for i in 0..1000 {
            let input = (i as f32 * 0.01).sin();
            let output = eq.process(input);
            assert!(
                (output - input).abs() < 1e-6,
                "Flat EQ should pass through unchanged at sample {i}: input={input}, output={output}"
            );
        }
    }

    #[test]
    fn boost_increases_energy() {
        // Boost 1 kHz band (index 8) by 12 dB
        let mut gains = flat_gains();
        gains[8] = 12.0;
        let mut eq = EqStage::new(gains, SAMPLE_RATE);

        // Generate a 1 kHz sine wave
        let freq = 1000.0_f32;
        let mut energy_in = 0.0_f64;
        let mut energy_out = 0.0_f64;
        let num_samples = SAMPLE_RATE as usize;

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let input = (2.0 * std::f32::consts::PI * freq * t).sin();
            let output = eq.process(input);
            energy_in += f64::from(input).powi(2);
            energy_out += f64::from(output).powi(2);
        }

        assert!(
            energy_out > energy_in * 2.0,
            "12 dB boost at 1 kHz should significantly increase energy: in={energy_in}, out={energy_out}"
        );
    }

    #[test]
    fn cut_decreases_energy() {
        // Cut 1 kHz band (index 8) by 12 dB
        let mut gains = flat_gains();
        gains[8] = -12.0;
        let mut eq = EqStage::new(gains, SAMPLE_RATE);

        let freq = 1000.0_f32;
        let mut energy_in = 0.0_f64;
        let mut energy_out = 0.0_f64;
        let num_samples = SAMPLE_RATE as usize;

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE;
            let input = (2.0 * std::f32::consts::PI * freq * t).sin();
            let output = eq.process(input);
            energy_in += f64::from(input).powi(2);
            energy_out += f64::from(output).powi(2);
        }

        assert!(
            energy_out < energy_in * 0.5,
            "12 dB cut at 1 kHz should significantly decrease energy: in={energy_in}, out={energy_out}"
        );
    }

    #[test]
    fn parameter_validation() {
        let mut eq = EqStage::new(flat_gains(), SAMPLE_RATE);

        // Valid parameters
        assert!(eq.set_parameter("band_0", 6.0).is_ok());
        assert!(eq.set_parameter("band_15", -6.0).is_ok());
        assert!((eq.get_parameter("band_0").unwrap() - 6.0).abs() < 1e-6);
        assert!((eq.get_parameter("band_15").unwrap() - (-6.0)).abs() < 1e-6);

        // Out of range gain
        assert!(eq.set_parameter("band_0", 13.0).is_err());
        assert!(eq.set_parameter("band_0", -13.0).is_err());

        // Invalid band index
        assert!(eq.set_parameter("band_16", 0.0).is_err());
        assert!(eq.get_parameter("band_16").is_err());

        // Unknown parameter name
        assert!(eq.set_parameter("volume", 0.0).is_err());
        assert!(eq.get_parameter("volume").is_err());
    }

    #[test]
    fn denormal_flushing() {
        let mut eq = EqStage::new(flat_gains(), SAMPLE_RATE);

        // Feed very small signal, then silence — state should not accumulate denormals
        for _ in 0..100 {
            eq.process(1e-30);
        }
        for _ in 0..1000 {
            let out = eq.process(0.0);
            assert!(
                out == 0.0 || out.abs() >= f32::MIN_POSITIVE,
                "Should not produce denormal values, got {out}"
            );
        }
    }

    #[test]
    fn high_sample_rate() {
        // Test at oversampled rate (e.g., 16x)
        let high_rate = 44100.0 * 16.0;
        let mut gains = flat_gains();
        gains[15] = 6.0; // Boost 20 kHz band
        let mut eq = EqStage::new(gains, high_rate);

        // Should not produce NaN or Inf
        for i in 0..10000 {
            let input = (i as f32 * 0.001).sin();
            let output = eq.process(input);
            assert!(output.is_finite(), "Output should be finite at sample {i}");
        }
    }

    #[test]
    fn all_bands_param_round_trip() {
        let mut eq = EqStage::new(flat_gains(), SAMPLE_RATE);

        for i in 0..NUM_BANDS {
            let name = EqStage::band_param_name(i);
            let gain = i as f32 - 8.0; // -8 to +7 dB
            eq.set_parameter(&name, gain).unwrap();
            let read = eq.get_parameter(&name).unwrap();
            assert!(
                (read - gain).abs() < 1e-6,
                "Band {i}: set {gain}, got {read}"
            );
        }
    }

    #[test]
    fn block_processing() {
        let mut gains = flat_gains();
        gains[4] = 6.0;

        let mut eq_single = EqStage::new(gains, SAMPLE_RATE);
        let mut eq_block = EqStage::new(gains, SAMPLE_RATE);

        // Generate test signal
        let mut signal: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let reference: Vec<f32> = signal.iter().map(|&s| eq_single.process(s)).collect();

        // Process as block
        eq_block.process_block(&mut signal);

        for (i, (got, want)) in signal.iter().zip(reference.iter()).enumerate() {
            assert!(
                (got - want).abs() < 1e-6,
                "Block/single mismatch at sample {i}: block={got}, single={want}"
            );
        }
    }

    #[test]
    fn stability_at_all_extremes() {
        // All bands at max boost
        let gains = [MAX_GAIN_DB; NUM_BANDS];
        let mut eq = EqStage::new(gains, SAMPLE_RATE);

        for i in 0..10000 {
            let input = if i % 100 == 0 { 1.0 } else { 0.0 };
            let output = eq.process(input);
            assert!(
                output.is_finite(),
                "Output must be finite at sample {i}, got {output}"
            );
        }

        // All bands at max cut — this is the scenario that caused rumbling with f32
        let gains = [MIN_GAIN_DB; NUM_BANDS];
        let mut eq = EqStage::new(gains, SAMPLE_RATE);

        for i in 0..10000 {
            let input = (i as f32 * 0.1).sin();
            let output = eq.process(input);
            assert!(
                output.is_finite(),
                "Output must be finite at sample {i}, got {output}"
            );
        }
    }

    #[test]
    fn low_band_cut_high_sample_rate() {
        // The exact failure case: 25 Hz band at -12 dB with 16x oversampling
        let high_rate = 44100.0 * 16.0;
        let mut gains = flat_gains();
        gains[0] = MIN_GAIN_DB; // 25 Hz band fully cut
        let mut eq = EqStage::new(gains, high_rate);

        let mut max_out: f32 = 0.0;
        for i in 0..100_000 {
            let input = (i as f32 * 0.01).sin() * 0.5;
            let output = eq.process(input);
            assert!(
                output.is_finite(),
                "Output must be finite at sample {i}, got {output}"
            );
            max_out = max_out.max(output.abs());
        }
        // Output should stay bounded — no rumbling or blowup
        assert!(
            max_out < 2.0,
            "25 Hz cut should not amplify signal, got max {max_out}"
        );
    }

    #[test]
    fn extreme_gain_high_sample_rate() {
        // High oversampling + extreme gain on all bands
        let high_rate = 44100.0 * 16.0;
        let gains = [MAX_GAIN_DB; NUM_BANDS];
        let mut eq = EqStage::new(gains, high_rate);

        for i in 0..50000 {
            let input = (i as f32 * 0.01).sin() * 0.5;
            let output = eq.process(input);
            assert!(
                output.is_finite(),
                "Output must be finite at high SR sample {i}, got {output}"
            );
        }
    }

    #[test]
    fn per_band_alpha_is_finite() {
        // Verify that alpha computation produces valid values for all bands
        // at both standard and oversampled rates
        for &sr in &[44100.0_f32, 48000.0, 44100.0 * 16.0] {
            let mut gains = flat_gains();
            gains[0] = 6.0; // low band
            gains[15] = 6.0; // high band
            let mut eq = EqStage::new(gains, sr);

            // Process a few samples — if alpha was bad, output goes NaN quickly
            for i in 0..1000 {
                let input = (i as f32 * 0.01).sin();
                let output = eq.process(input);
                assert!(
                    output.is_finite(),
                    "Output must be finite at SR={sr}, sample {i}"
                );
            }
        }
    }
}
