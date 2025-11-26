use anyhow::Result;
use log::warn;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::path::Path;
use std::sync::Arc;

use crate::ir::{loader::IrLoader, model::ImpulseResponse};

const FFT_BLOCK_SIZE: usize = 1024;
const PARTITION_SIZE: usize = FFT_BLOCK_SIZE / 2;
// Zero-latency head length (time-domain)
const HEAD_LEN: usize = 256;
const TAIL_OFFSET_SAMPLES: usize = HEAD_LEN % FFT_BLOCK_SIZE;
const MAX_PARTITIONS: usize = 40;

pub struct IrCabinet {
    ir_loader: IrLoader,
    current_ir: Option<ImpulseResponse>,

    // FFT
    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,

    // Input time buffer
    input_buffer: Vec<f32>,
    in_base: usize,
    in_pos: usize,

    // Frequency-domain input history ring
    history: Vec<Vec<Complex<f32>>>,
    hist_head: usize,

    // Overlap-add circular buffer
    ola_buf: Vec<f32>,
    ola_w: usize,
    ola_r: usize,

    // Scratch buffers
    time_scratch: Vec<f32>,
    freq_scratch: Vec<Complex<f32>>,
    freq_accumulator: Vec<Complex<f32>>,
    r2c_scratch: Vec<realfft::num_complex::Complex32>,
    c2r_scratch: Vec<realfft::num_complex::Complex32>,

    // Head FIR input ring
    head_ring: Vec<f32>,
    head_w: usize,

    bypassed: bool,

    output_gain: f32,
    dc_prev_x: f32,
    dc_prev_y: f32,
    dc_r: f32,

    tail_mix: f32,
}

impl IrCabinet {
    pub fn new(ir_directory: &Path, sample_rate: usize) -> Result<Self> {
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(FFT_BLOCK_SIZE);
        let c2r = planner.plan_fft_inverse(FFT_BLOCK_SIZE);
        let r2c_scratch = r2c.make_scratch_vec();
        let c2r_scratch = c2r.make_scratch_vec();

        Ok(Self {
            ir_loader: IrLoader::new(ir_directory, sample_rate)?,
            current_ir: None,
            r2c,
            c2r,

            input_buffer: vec![0.0; FFT_BLOCK_SIZE],
            in_base: 0,
            in_pos: 0,

            history: Vec::new(),
            hist_head: 0,

            ola_buf: vec![0.0; FFT_BLOCK_SIZE],
            ola_w: 0,
            ola_r: 0,

            time_scratch: vec![0.0; FFT_BLOCK_SIZE],
            freq_scratch: vec![Complex::new(0.0, 0.0); FFT_BLOCK_SIZE / 2 + 1],
            freq_accumulator: vec![Complex::new(0.0, 0.0); FFT_BLOCK_SIZE / 2 + 1],
            r2c_scratch,
            c2r_scratch,

            head_ring: vec![0.0; HEAD_LEN],
            head_w: 0,

            bypassed: false,

            output_gain: 0.1,
            dc_prev_x: 0.0,
            dc_prev_y: 0.0,
            dc_r: 0.995,

            tail_mix: 0.1,
        })
    }

    pub fn available_ir_names(&self) -> Vec<String> {
        self.ir_loader.available_ir_names()
    }

    pub fn select_ir(&mut self, name: &str) -> Result<()> {
        let ir_sample = self.ir_loader.load_by_name(name)?;

        let ir = self.create_response(ir_sample)?;

        self.current_ir = Some(ir);
        self.reset_buffers();
        Ok(())
    }

    fn create_response(&mut self, ir_sample: Vec<f32>) -> Result<ImpulseResponse> {
        const MAX_IR_LENGTH: usize = 96000;
        let mut truncated: Vec<f32> = ir_sample.into_iter().take(MAX_IR_LENGTH).collect();

        let mut end = truncated.len();
        while end > 0 && truncated[end - 1].abs() < 1.0e-5 {
            end -= 1;
        }
        truncated.truncate(end.max(HEAD_LEN));

        let head_len = truncated.len().min(HEAD_LEN);
        let mut head = vec![0.0f32; HEAD_LEN];
        head[..head_len].copy_from_slice(&truncated[..head_len]);

        let tail = if truncated.len() > HEAD_LEN {
            &truncated[HEAD_LEN..]
        } else {
            &[][..]
        };

        let tail_partitions = self.partition_ir_tail(tail)?;
        let num_tail_partitions = tail_partitions.len();

        Ok(ImpulseResponse {
            head_coeffs: head,
            tail_partitions,
            num_tail_partitions,
            original_length: truncated.len(),
        })
    }

    fn partition_ir_tail(&mut self, tail_samples: &[f32]) -> Result<Vec<Vec<Complex<f32>>>> {
        if tail_samples.is_empty() {
            return Ok(Vec::new());
        }

        let max_samples = MAX_PARTITIONS * PARTITION_SIZE;
        let truncated = if tail_samples.len() > max_samples {
            warn!(
                "IR tail truncated from {} to {} samples for performance",
                tail_samples.len(),
                max_samples
            );
            &tail_samples[..max_samples]
        } else {
            tail_samples
        };

        let num_partitions = truncated.len().div_ceil(PARTITION_SIZE);
        let mut parts = Vec::with_capacity(num_partitions);

        for p in 0..num_partitions {
            let start = p * PARTITION_SIZE;
            let end = ((p + 1) * PARTITION_SIZE).min(tail_samples.len());

            let mut time_block = vec![0.0f32; FFT_BLOCK_SIZE];
            for (i, idx) in (start..end).enumerate() {
                time_block[i] = tail_samples[idx];
            }

            let mut freq_block = vec![Complex::new(0.0, 0.0); FFT_BLOCK_SIZE / 2 + 1];

            self.r2c
                .process(&mut time_block, &mut freq_block)
                .expect("realfft forward failed");

            parts.push(freq_block);
        }

        Ok(parts)
    }

    fn reset_buffers(&mut self) {
        self.input_buffer.fill(0.0);
        self.in_base = 0;
        self.in_pos = 0;

        self.ola_buf.fill(0.0);
        self.ola_w = 0;
        self.ola_r = 0;

        self.time_scratch.fill(0.0);
        self.freq_scratch.fill(Complex::new(0.0, 0.0));
        self.freq_accumulator.fill(Complex::new(0.0, 0.0));

        self.head_ring.fill(0.0);
        self.head_w = 0;

        if let Some(ref ir) = self.current_ir {
            let bins = FFT_BLOCK_SIZE / 2 + 1;
            self.history = vec![vec![Complex::new(0.0, 0.0); bins]; ir.num_tail_partitions];
            self.hist_head = 0;

            self.tail_mix = if ir.original_length > 60000 {
                0.10
            } else if ir.original_length > 30000 {
                0.20
            } else {
                0.35
            };
        } else {
            self.history.clear();
            self.hist_head = 0;
            self.tail_mix = 0.35;
        }
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        if self.bypassed || self.current_ir.is_none() {
            return;
        }
        for s in samples.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    #[inline]
    fn process_sample(&mut self, input: f32) -> f32 {
        let ir = match self.current_ir.as_ref() {
            Some(ir) => ir,
            None => return 0.0,
        };

        // Head FIR
        self.head_ring[self.head_w] = input;
        let mut head_out = 0.0f32;

        let mut idx = self.head_w;
        for &h in ir.head_coeffs.iter() {
            head_out += h * self.head_ring[idx];
            idx = if idx == 0 { HEAD_LEN - 1 } else { idx - 1 };
        }
        self.head_w = (self.head_w + 1) % HEAD_LEN;

        // Tail path
        let write_off = PARTITION_SIZE + self.in_pos;
        let widx = (self.in_base + write_off) % FFT_BLOCK_SIZE;
        self.input_buffer[widx] = input;

        let tail_out = zap_denormal(self.ola_buf[self.ola_r]);
        self.ola_buf[self.ola_r] = 0.0;
        self.ola_r = (self.ola_r + 1) % FFT_BLOCK_SIZE;

        self.in_pos += 1;
        if self.in_pos >= PARTITION_SIZE {
            self.process_tail_partition();
            self.in_pos = 0;
            self.in_base = (self.in_base + PARTITION_SIZE) % FFT_BLOCK_SIZE;
        }

        let mut y = head_out + self.tail_mix * tail_out;

        // DC blocker
        let dc = y - self.dc_prev_x + self.dc_r * self.dc_prev_y;
        self.dc_prev_x = y;
        self.dc_prev_y = dc;

        y = dc * self.output_gain;

        y
    }

    fn process_tail_partition(&mut self) {
        let ir = match self.current_ir.as_ref() {
            Some(ir) => ir,
            None => return,
        };

        if ir.num_tail_partitions == 0 {
            return;
        }

        for i in 0..FFT_BLOCK_SIZE {
            let idx = (self.in_base + i) % FFT_BLOCK_SIZE;
            self.time_scratch[i] = self.input_buffer[idx];
        }

        self.r2c
            .process_with_scratch(
                &mut self.time_scratch,
                &mut self.freq_scratch,
                &mut self.r2c_scratch,
            )
            .expect("realfft forward failed");

        if !self.history.is_empty() {
            self.history[self.hist_head].copy_from_slice(&self.freq_scratch);
            self.hist_head = (self.hist_head + 1) % self.history.len();
        }

        self.freq_accumulator.fill(Complex::new(0.0, 0.0));

        let num_partitions = ir.tail_partitions.len();
        for j in 0..num_partitions {
            // Read from history in reverse time order
            let history_idx = (self.hist_head + self.history.len() - 1 - j) % self.history.len();

            let x = &self.history[history_idx];
            let h = &ir.tail_partitions[j];

            for k in 0..self.freq_accumulator.len() {
                let prod = x[k] * h[k];
                self.freq_accumulator[k] +=
                    Complex::new(zap_denormal(prod.re), zap_denormal(prod.im));
            }
        }

        self.freq_accumulator[0].im = 0.0;
        if let Some(last) = self.freq_accumulator.last_mut() {
            last.im = 0.0;
        }

        self.c2r
            .process_with_scratch(
                &mut self.freq_accumulator,
                &mut self.time_scratch,
                &mut self.c2r_scratch,
            )
            .expect("realfft inverse failed");

        let scale = 1.0 / FFT_BLOCK_SIZE as f32;
        let base = (self.ola_w + TAIL_OFFSET_SAMPLES) % FFT_BLOCK_SIZE;
        for i in 0..FFT_BLOCK_SIZE {
            let pos = (base + i) % FFT_BLOCK_SIZE;
            let v = zap_denormal(self.time_scratch[i] * scale);
            self.ola_buf[pos] += v;
        }

        self.ola_w = (self.ola_w + PARTITION_SIZE) % FFT_BLOCK_SIZE;
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
        if bypass {
            self.reset_buffers();
        }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.output_gain = gain.clamp(0.0, 1.0);
    }
}

#[inline]
fn zap_denormal(x: f32) -> f32 {
    if x.abs() < 1.0e-30 { 0.0 } else { x }
}
