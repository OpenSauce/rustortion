use anyhow::Result;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

/// Default head length for zero-latency time-domain processing
const HEAD_LEN: usize = 256;
/// FFT block size
const FFT_BLOCK_SIZE: usize = 1024;

/// Two-stage convolver: time-domain FIR for the head (zero latency),
/// partitioned FFT convolution for the tail.
///
/// This is the Gardner method for low-latency convolution.
pub struct TwoStageConvolver {
    /// FFT block size for tail processing
    block_size: usize,
    /// Partition size (block_size / 2 for overlap-save)
    partition_size: usize,
    /// Number of frequency bins
    num_bins: usize,

    // Head (time-domain FIR)
    head_coeffs: Vec<f32>,
    head_ring: Vec<f32>,
    head_write_pos: usize,

    // Tail (FFT partitioned convolution)
    tail_partitions: Vec<Vec<Complex<f32>>>,
    num_tail_partitions: usize,

    // FFT planners
    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,

    // Input buffer for tail processing
    input_buffer: Vec<f32>,
    input_base: usize,
    input_pos: usize,

    // Frequency-domain input history
    history: Vec<Vec<Complex<f32>>>,
    history_head: usize,

    // Overlap-add buffer for tail output
    ola_buffer: Vec<f32>,
    ola_write: usize,
    ola_read: usize,

    // Scratch buffers
    time_scratch: Vec<f32>,
    freq_scratch: Vec<Complex<f32>>,
    freq_accumulator: Vec<Complex<f32>>,
    r2c_scratch: Vec<Complex<f32>>,
    c2r_scratch: Vec<Complex<f32>>,
}

impl Default for TwoStageConvolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoStageConvolver {
    pub fn new() -> Self {
        let block_size = FFT_BLOCK_SIZE;
        assert!(
            block_size.is_power_of_two(),
            "block_size must be power of 2"
        );

        let partition_size = block_size / 2;
        let num_bins = block_size / 2 + 1;

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(block_size);
        let c2r = planner.plan_fft_inverse(block_size);
        let r2c_scratch = r2c.make_scratch_vec();
        let c2r_scratch = c2r.make_scratch_vec();

        Self {
            block_size,
            partition_size,
            num_bins,

            head_coeffs: vec![0.0; HEAD_LEN],
            head_ring: vec![0.0; HEAD_LEN],
            head_write_pos: 0,

            tail_partitions: Vec::new(),
            num_tail_partitions: 0,

            r2c,
            c2r,

            input_buffer: vec![0.0; block_size],
            input_base: 0,
            input_pos: 0,

            history: Vec::new(),
            history_head: 0,

            ola_buffer: vec![0.0; block_size],
            ola_write: 0,
            ola_read: 0,

            time_scratch: vec![0.0; block_size],
            freq_scratch: vec![Complex::new(0.0, 0.0); num_bins],
            freq_accumulator: vec![Complex::new(0.0, 0.0); num_bins],
            r2c_scratch,
            c2r_scratch,
        }
    }

    pub fn set_ir(&mut self, ir: &[f32]) -> Result<()> {
        if ir.is_empty() {
            self.head_coeffs.fill(0.0);
            self.tail_partitions.clear();
            self.num_tail_partitions = 0;
            self.history.clear();
            return Ok(());
        }

        // Split IR into head and tail
        let head_len = ir.len().min(HEAD_LEN);
        self.head_coeffs = vec![0.0; HEAD_LEN];
        self.head_coeffs[..head_len].copy_from_slice(&ir[..head_len]);

        // Partition tail for FFT convolution
        if ir.len() > HEAD_LEN {
            let tail = &ir[HEAD_LEN..];
            self.partition_tail(tail)?;
        } else {
            self.tail_partitions.clear();
            self.num_tail_partitions = 0;
            self.history.clear();
        }

        // Reset state
        self.reset();

        Ok(())
    }

    fn partition_tail(&mut self, tail: &[f32]) -> Result<()> {
        self.num_tail_partitions = tail.len().div_ceil(self.partition_size);
        self.tail_partitions = Vec::with_capacity(self.num_tail_partitions);

        for p in 0..self.num_tail_partitions {
            let start = p * self.partition_size;
            let end = (start + self.partition_size).min(tail.len());

            // Zero-pad to block_size
            let mut time_block = vec![0.0f32; self.block_size];
            for (i, idx) in (start..end).enumerate() {
                time_block[i] = tail[idx];
            }

            // Transform to frequency domain
            let mut freq_block = vec![Complex::new(0.0, 0.0); self.num_bins];
            self.r2c
                .process_with_scratch(&mut time_block, &mut freq_block, &mut self.r2c_scratch)
                .map_err(|e| anyhow::anyhow!("FFT failed during IR partitioning: {e}"))?;

            self.tail_partitions.push(freq_block);
        }

        // Initialize history buffer
        self.history = vec![vec![Complex::new(0.0, 0.0); self.num_bins]; self.num_tail_partitions];
        self.history_head = 0;

        Ok(())
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // === Head processing (zero latency) ===
        self.head_ring[self.head_write_pos] = input;

        let mut head_out = 0.0f32;
        let mut idx = self.head_write_pos;

        for &coeff in &self.head_coeffs {
            head_out += coeff * self.head_ring[idx];
            if idx == 0 {
                idx = HEAD_LEN - 1;
            } else {
                idx -= 1;
            }
        }

        self.head_write_pos = (self.head_write_pos + 1) % HEAD_LEN;

        // === Tail processing (FFT with latency) ===
        let tail_out = if self.num_tail_partitions > 0 {
            // Write input to buffer
            let write_idx =
                (self.input_base + self.partition_size + self.input_pos) % self.block_size;
            self.input_buffer[write_idx] = input;

            // Read from overlap-add buffer
            let out = self.ola_buffer[self.ola_read];
            self.ola_buffer[self.ola_read] = 0.0;
            self.ola_read = (self.ola_read + 1) % self.block_size;

            self.input_pos += 1;

            // Process partition when we've collected enough samples
            if self.input_pos >= self.partition_size {
                self.process_tail_partition();
                self.input_pos = 0;
                self.input_base = (self.input_base + self.partition_size) % self.block_size;
            }

            out
        } else {
            0.0
        };

        head_out + tail_out
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    fn process_tail_partition(&mut self) {
        // Copy input buffer to FFT input
        for i in 0..self.block_size {
            let idx = (self.input_base + i) % self.block_size;
            self.time_scratch[i] = self.input_buffer[idx];
        }

        // Forward FFT
        if self
            .r2c
            .process_with_scratch(
                &mut self.time_scratch,
                &mut self.freq_scratch,
                &mut self.r2c_scratch,
            )
            .is_err()
        {
            // Advance ola_write to keep buffer alignment on error.
            self.ola_write = (self.ola_write + self.partition_size) % self.block_size;
            return;
        }

        // Store in history
        self.history[self.history_head].copy_from_slice(&self.freq_scratch);
        self.history_head = (self.history_head + 1) % self.history.len();

        // Accumulate convolution
        self.freq_accumulator.fill(Complex::new(0.0, 0.0));

        for j in 0..self.num_tail_partitions {
            let hist_idx = (self.history_head + self.history.len() - 1 - j) % self.history.len();

            for (k, acc) in self.freq_accumulator.iter_mut().enumerate() {
                *acc += self.history[hist_idx][k] * self.tail_partitions[j][k];
            }
        }

        // Ensure DC and Nyquist are real
        self.freq_accumulator[0].im = 0.0;
        if let Some(last) = self.freq_accumulator.last_mut() {
            last.im = 0.0;
        }

        // Inverse FFT
        if self
            .c2r
            .process_with_scratch(
                &mut self.freq_accumulator,
                &mut self.time_scratch,
                &mut self.c2r_scratch,
            )
            .is_err()
        {
            // Advance ola_write to keep buffer alignment on error.
            self.ola_write = (self.ola_write + self.partition_size) % self.block_size;
            return;
        }

        // Overlap-add into output buffer
        let scale = 1.0 / self.block_size as f32;
        let tail_offset = HEAD_LEN % self.block_size;
        let base = (self.ola_write + tail_offset) % self.block_size;

        for i in 0..self.block_size {
            let pos = (base + i) % self.block_size;
            self.ola_buffer[pos] += self.time_scratch[i] * scale;
        }

        self.ola_write = (self.ola_write + self.partition_size) % self.block_size;
    }

    pub fn reset(&mut self) {
        self.head_ring.fill(0.0);
        self.head_write_pos = 0;

        self.input_buffer.fill(0.0);
        self.input_base = 0;
        self.input_pos = 0;

        self.ola_buffer.fill(0.0);
        self.ola_write = 0;
        self.ola_read = 0;

        self.history_head = 0;
        for hist in &mut self.history {
            hist.fill(Complex::new(0.0, 0.0));
        }

        self.time_scratch.fill(0.0);
        self.freq_scratch.fill(Complex::new(0.0, 0.0));
        self.freq_accumulator.fill(Complex::new(0.0, 0.0));
    }

    pub const fn num_tail_partitions(&self) -> usize {
        self.num_tail_partitions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_stage_short_ir() {
        // IR shorter than HEAD_LEN - should only use head
        let mut conv = TwoStageConvolver::new();
        conv.set_ir(&[1.0, 0.5, 0.25]).unwrap();

        assert_eq!(conv.num_tail_partitions(), 0);

        // Test impulse response
        let y0 = conv.process_sample(1.0);
        let y1 = conv.process_sample(0.0);
        let y2 = conv.process_sample(0.0);

        assert!((y0 - 1.0).abs() < 1e-5);
        assert!((y1 - 0.5).abs() < 1e-5);
        assert!((y2 - 0.25).abs() < 1e-5);
    }

    #[test]
    fn test_two_stage_long_ir() {
        // IR longer than HEAD_LEN - uses both head and tail
        let mut conv = TwoStageConvolver::new();

        let long_ir: Vec<f32> = (0..1000).map(|i| 1.0 / (i + 1) as f32).collect();
        conv.set_ir(&long_ir).unwrap();

        // Should have tail partitions
        assert!(conv.num_tail_partitions() > 0);
    }
}
