use anyhow::Result;

/// Simple time-domain FIR convolver using a ring buffer.
///
/// Best for short IRs (< 50ms / ~2400 samples at 48kHz).
/// Zero latency, O(buffer_size * ir_length) complexity.
pub struct FirConvolver {
    /// IR coefficients (stored in original order)
    coefficients: Vec<f32>,
    /// Ring buffer for input history
    input_buffer: Vec<f32>,
    /// Current write position in ring buffer
    write_pos: usize,
    /// Maximum IR length this convolver supports
    max_length: usize,
}

impl FirConvolver {
    pub fn new(max_length: usize) -> Self {
        Self {
            coefficients: Vec::new(),
            input_buffer: vec![0.0; max_length],
            write_pos: 0,
            max_length,
        }
    }

    pub fn set_ir(&mut self, ir: &[f32]) -> Result<()> {
        // Truncate IR if longer than max
        let truncated_len = ir.len().min(self.max_length);

        // Copy and truncate
        self.coefficients = ir[..truncated_len].to_vec();

        // Resize input buffer to match IR length
        self.input_buffer = vec![0.0; truncated_len];
        self.write_pos = 0;

        Ok(())
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if self.coefficients.is_empty() {
            return input;
        }

        let len = self.coefficients.len();

        // Write current sample
        self.input_buffer[self.write_pos] = input;

        let mut output = 0.0;

        let mut idx = self.write_pos;

        for &coeff in &self.coefficients {
            output += self.input_buffer[idx] * coeff;
            if idx == 0 {
                idx = len - 1;
            } else {
                idx -= 1;
            }
        }

        // Advance write position
        self.write_pos = (self.write_pos + 1) % len;

        output
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    pub fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.write_pos = 0;
    }

    pub fn latency(&self) -> usize {
        0 // FIR has zero latency
    }

    /// Returns the current IR length
    pub fn ir_length(&self) -> usize {
        self.coefficients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fir_impulse_response() {
        // Test with a simple IR: [1.0, 0.5, 0.25]
        let mut conv = FirConvolver::new(1024);
        conv.set_ir(&[1.0, 0.5, 0.25]).unwrap();

        // Feed an impulse (1.0 followed by zeros)
        let y0 = conv.process_sample(1.0);
        let y1 = conv.process_sample(0.0);
        let y2 = conv.process_sample(0.0);
        let y3 = conv.process_sample(0.0);

        // Output should be the IR itself
        assert!((y0 - 1.0).abs() < 1e-6);
        assert!((y1 - 0.5).abs() < 1e-6);
        assert!((y2 - 0.25).abs() < 1e-6);
        assert!((y3 - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_fir_truncation() {
        let mut conv = FirConvolver::new(100);
        let long_ir: Vec<f32> = (0..500).map(|i| i as f32 * 0.001).collect();

        conv.set_ir(&long_ir).unwrap();

        assert_eq!(conv.ir_length(), 100);
    }

    #[test]
    fn test_fir_reset() {
        let mut conv = FirConvolver::new(1024);
        conv.set_ir(&[1.0, 0.5, 0.25]).unwrap();

        // Process some samples
        conv.process_sample(1.0);
        conv.process_sample(0.5);

        // Reset
        conv.reset();

        // After reset, processing zero should output zero
        let y = conv.process_sample(0.0);
        assert!((y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_fir_block_processing() {
        let mut conv = FirConvolver::new(1024);
        conv.set_ir(&[1.0, 0.5]).unwrap();

        let mut block = [1.0, 0.0, 0.0, 0.0];
        conv.process_block(&mut block);

        assert!((block[0] - 1.0).abs() < 1e-6);
        assert!((block[1] - 0.5).abs() < 1e-6);
        assert!((block[2] - 0.0).abs() < 1e-6);
    }
}
