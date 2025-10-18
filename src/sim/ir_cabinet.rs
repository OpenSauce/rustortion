use anyhow::{Context, Result};
use hound::WavReader;
use log::{debug, info, warn};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FFT_BLOCK_SIZE: usize = 2048;
const PARTITION_SIZE: usize = FFT_BLOCK_SIZE / 2;

// Zero-latency head length (time-domain)
const HEAD_LEN: usize = 256;
const TAIL_OFFSET_SAMPLES: usize = HEAD_LEN % FFT_BLOCK_SIZE;
const TAIL_MIX: f32 = 0.35;

#[derive(Clone)]
pub struct ImpulseResponse {
    pub name: String,
    pub path: PathBuf,

    // Zero-latency head
    pub head_coeffs: Vec<f32>,

    // Partitioned FFT tail
    pub tail_partitions: Vec<Vec<Complex<f32>>>,
    pub num_tail_partitions: usize,

    pub original_length: usize,
    pub sample_rate: u32,
}

pub struct IrCabinet {
    current_ir: Option<ImpulseResponse>,
    // Just store paths, not loaded IRs
    available_ir_paths: Vec<(String, PathBuf)>, // (display_name, full_path)
    ir_directory: PathBuf,
    target_sample_rate: u32,

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
}

impl IrCabinet {
    pub fn new(ir_directory: &Path, sample_rate: u32) -> Result<Self> {
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(FFT_BLOCK_SIZE);
        let c2r = planner.plan_fft_inverse(FFT_BLOCK_SIZE);
        let r2c_scratch = r2c.make_scratch_vec();
        let c2r_scratch = c2r.make_scratch_vec();

        let mut cabinet = Self {
            current_ir: None,
            available_ir_paths: Vec::new(),
            ir_directory: ir_directory.to_path_buf(),
            target_sample_rate: sample_rate,
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

            output_gain: 0.5,
            dc_prev_x: 0.0,
            dc_prev_y: 0.0,
            dc_r: 0.995,
        };

        cabinet.scan_ir_directory()?;

        if !cabinet.available_ir_paths.is_empty() {
            cabinet.set_ir_by_index(0)?;
        }

        Ok(cabinet)
    }

    pub fn scan_ir_directory(&mut self) -> Result<()> {
        if !self.ir_directory.exists() {
            fs::create_dir_all(&self.ir_directory).context("Failed to create IR directory")?;
            warn!("IR directory created at {:?}", self.ir_directory);
            return Ok(());
        }

        self.available_ir_paths.clear();
        let base = self.ir_directory.clone();
        self.scan_recursive(&base, &base)?;

        self.available_ir_paths.sort_by(|a, b| {
            let a_sep_count = a.0.matches('/').count();
            let b_sep_count = b.0.matches('/').count();
            a_sep_count.cmp(&b_sep_count).then_with(|| a.0.cmp(&b.0))
        });

        info!(
            "Found {} impulse response files",
            self.available_ir_paths.len()
        );
        Ok(())
    }

    fn scan_recursive(&mut self, current_dir: &Path, base_dir: &Path) -> Result<()> {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_recursive(&path, base_dir)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                let relative_path = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");

                self.available_ir_paths.push((relative_path, path));
            }
        }
        Ok(())
    }

    pub fn set_ir_by_index(&mut self, index: usize) -> Result<()> {
        if index >= self.available_ir_paths.len() {
            return Err(anyhow::anyhow!("IR index out of range"));
        }

        let (display_name, path) = self.available_ir_paths[index].clone();
        info!("Loading IR: {}", display_name);

        let ir = self.load_ir_file(&path, &display_name)?;

        info!(
            "Loaded IR: {} (head {} taps, tail {} partitions, {} samples)",
            ir.name,
            ir.head_coeffs.len(),
            ir.num_tail_partitions,
            ir.original_length
        );

        self.current_ir = Some(ir);
        self.reset_buffers();
        Ok(())
    }

    pub fn set_ir_by_name(&mut self, name: &str) -> Result<()> {
        let index = self
            .available_ir_paths
            .iter()
            .position(|(n, _)| n == name)
            .ok_or_else(|| anyhow::anyhow!("IR '{}' not found", name))?;

        self.set_ir_by_index(index)
    }

    fn load_ir_file(&mut self, path: &Path, display_name: &str) -> Result<ImpulseResponse> {
        let reader = WavReader::open(path).context("Failed to open WAV file")?;
        let spec = reader.spec();

        let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader
                .into_samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read float samples")?
        } else {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read integer samples")?
        };

        let mono = if spec.channels > 1 {
            samples
                .chunks(spec.channels as usize)
                .map(|c| c.iter().sum::<f32>() / spec.channels as f32)
                .collect()
        } else {
            samples
        };

        let resampled = if spec.sample_rate != self.target_sample_rate {
            debug!(
                "Resampling IR from {} Hz to {} Hz",
                spec.sample_rate, self.target_sample_rate
            );
            resample_linear(&mono, spec.sample_rate, self.target_sample_rate)
        } else {
            mono
        };

        const MAX_IR_LENGTH: usize = 96000;
        let mut truncated: Vec<f32> = resampled.into_iter().take(MAX_IR_LENGTH).collect();

        // Normalize with headroom
        if let Some(max) = truncated.iter().fold(None::<f32>, |m, &x| {
            Some(m.map_or(x.abs(), |mm| mm.max(x.abs())))
        }) && max > 0.0
        {
            let g = 0.9 / max;
            for s in &mut truncated {
                *s *= g;
            }
        }

        let mut end = truncated.len();
        while end > 0 && truncated[end - 1].abs() < 1.0e-3 {
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
            name: display_name.to_string(),
            path: path.to_path_buf(),
            head_coeffs: head,
            tail_partitions,
            num_tail_partitions,
            original_length: truncated.len(),
            sample_rate: self.target_sample_rate,
        })
    }

    fn partition_ir_tail(&mut self, tail_samples: &[f32]) -> Result<Vec<Vec<Complex<f32>>>> {
        if tail_samples.is_empty() {
            return Ok(Vec::new());
        }
        let num_partitions = tail_samples.len().div_ceil(PARTITION_SIZE);
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
        } else {
            self.history.clear();
            self.hist_head = 0;
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

        let mut y = head_out + TAIL_MIX * tail_out;

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
        let hlen = self.history.len();
        let plen = ir.tail_partitions.len();
        let len = hlen.min(plen);

        for j in 0..len {
            let newest = (self.hist_head + hlen - 1) % hlen;
            let idx = (newest + hlen - j) % hlen;
            let x = &self.history[idx];
            let h = &ir.tail_partitions[j];

            for k in 0..self.freq_accumulator.len() {
                self.freq_accumulator[k] += x[k] * h[k];
            }
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

    pub fn get_available_irs(&self) -> Vec<String> {
        self.available_ir_paths
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn get_current_ir_name(&self) -> Option<String> {
        self.current_ir.as_ref().map(|ir| ir.name.clone())
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

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let s = if src_idx + 1 < samples.len() {
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };
        out.push(s);
    }
    out
}

#[inline]
fn zap_denormal(x: f32) -> f32 {
    if x.abs() < 1.0e-30 { 0.0 } else { x }
}
