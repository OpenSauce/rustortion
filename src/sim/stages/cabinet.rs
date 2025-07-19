use crate::sim::stages::Stage;
use log::debug;
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct CabinetStage {
    name: String,

    // FFT processing
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,

    // Pre-computed IR in frequency domain
    ir_spectrum: Vec<Complex32>,

    // Processing buffers
    input_buffer: Vec<f32>,
    overlap_buffer: Vec<f32>,
    fft_buffer: Vec<f32>,
    spectrum_buffer: Vec<Complex32>,
    output_buffer: Vec<f32>,
    output_pos: usize,

    // Configuration
    block_size: usize,
    buffer_pos: usize,
    enabled: bool,
}

impl CabinetStage {
    pub fn new(name: &str, ir_samples: Vec<f32>, block_size: usize) -> Self {
        let ir_length = ir_samples.len();

        // Use overlap-add method: FFT size = block_size + ir_length - 1
        // Round up to next power of 2 for efficiency
        let fft_size = (block_size + ir_length - 1).next_power_of_two();

        debug!(
            "Cabinet {}: IR length: {}, block_size: {}, FFT size: {}",
            name, ir_length, block_size, fft_size
        );

        // SAFETY: Prevent enormous FFTs
        if fft_size > 65536 {
            panic!("FFT size too large: {} (max 65536)", fft_size);
        }

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        // Pre-compute IR spectrum
        let mut ir_fft_input = vec![0.0; fft_size];
        ir_fft_input[..ir_length].copy_from_slice(&ir_samples);

        let mut ir_spectrum = fft.make_output_vec();
        fft.process(&mut ir_fft_input, &mut ir_spectrum).unwrap();

        debug!("IR spectrum magnitude at DC: {}", ir_spectrum[0].norm());
        debug!(
            "IR spectrum magnitude at Nyquist: {}",
            ir_spectrum.last().unwrap().norm()
        );

        Self {
            name: name.to_string(),
            fft,
            ifft,
            ir_spectrum,
            input_buffer: vec![0.0; block_size],
            overlap_buffer: vec![0.0; ir_length - 1],
            fft_buffer: vec![0.0; fft_size],
            spectrum_buffer: vec![Complex32::new(0.0, 0.0); fft_size / 2 + 1],
            output_buffer: vec![0.0; fft_size],
            output_pos: 0,
            block_size,
            buffer_pos: 0,
            enabled: true,
        }
    }

    pub fn load_from_wav(
        name: &str,
        wav_path: &str,
        block_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = hound::WavReader::open(wav_path)?;

        debug!("Loaded WAV file: {wav_path}");

        // Handle different sample formats
        let samples: Vec<f32> = match reader.spec().sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
            hound::SampleFormat::Int => {
                let bit_depth = reader.spec().bits_per_sample;
                match bit_depth {
                    16 => reader
                        .samples::<i16>()
                        .map(|s| s.map(|sample| sample as f32 / i16::MAX as f32))
                        .collect::<Result<Vec<_>, _>>()?,
                    24 => reader
                        .samples::<i32>()
                        .map(|s| s.map(|sample| sample as f32 / ((1 << 23) as f32)))
                        .collect::<Result<Vec<_>, _>>()?,
                    32 => reader
                        .samples::<i32>()
                        .map(|s| s.map(|sample| sample as f32 / i32::MAX as f32))
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return Err(format!("Unsupported bit depth: {bit_depth}").into()),
                }
            }
        };

        // Handle stereo by taking left channel or mixing to mono
        let mono_samples = if reader.spec().channels == 1 {
            samples
        } else {
            samples
                .chunks_exact(reader.spec().channels as usize)
                .map(|frame| frame[0]) // Take left channel
                .collect()
        };

        // Limit IR length for real-time performance (8192 samples ≈ 170ms at 48kHz)
        let max_ir_length = 8192;
        let final_samples = if mono_samples.len() > max_ir_length {
            mono_samples[..max_ir_length].to_vec()
        } else {
            mono_samples
        };

        Ok(Self::new(name, final_samples, block_size))
    }

    fn process_block(&mut self) {
        debug!("=== BLOCK START ===");

        // Check input sanity
        let input_finite = self.input_buffer.iter().all(|x| x.is_finite());
        debug!("Input buffer finite: {}", input_finite);
        if !input_finite {
            debug!("Non-finite input detected, clearing buffer");
            self.input_buffer.fill(0.0);
        }

        self.fft_buffer.fill(0.0);
        self.fft_buffer[..self.block_size].copy_from_slice(&self.input_buffer);
        debug!("FFT buffer prepared");

        // Forward FFT with error checking
        match self
            .fft
            .process(&mut self.fft_buffer, &mut self.spectrum_buffer)
        {
            Ok(_) => debug!("Forward FFT completed"),
            Err(e) => {
                debug!("Forward FFT failed: {:?}", e);
                return; // Skip this block
            }
        }

        // Check spectrum sanity
        let spectrum_finite = self
            .spectrum_buffer
            .iter()
            .all(|c| c.re.is_finite() && c.im.is_finite());
        debug!("Spectrum finite: {}", spectrum_finite);

        // Multiply by IR spectrum
        for (output, &ir) in self.spectrum_buffer.iter_mut().zip(self.ir_spectrum.iter()) {
            *output *= ir;
        }
        debug!("Spectrum multiplication completed");

        // Inverse FFT with error checking
        match self
            .ifft
            .process(&mut self.spectrum_buffer, &mut self.output_buffer)
        {
            Ok(_) => debug!("Inverse FFT completed"),
            Err(e) => {
                debug!("Inverse FFT failed: {:?}", e);
                return; // Skip this block
            }
        }

        // Check output sanity before normalization
        let output_finite = self.output_buffer.iter().all(|x| x.is_finite());
        debug!("Output before normalization finite: {}", output_finite);

        let max_out = self.output_buffer[..self.block_size]
            .iter()
            .map(|x| x.abs())
            .fold(0.0f32, |a, b| a.max(b));

        debug!("Max output level: {:.6}", max_out);

        // Normalization
        let scale = 1.0 / (self.output_buffer.len() as f32);
        debug!("Normalization scale: {}", scale);

        for sample in &mut self.output_buffer {
            *sample *= scale;
            if !sample.is_finite() {
                *sample = 0.0; // Safety
            }
        }
        debug!("Normalization completed");

        // Overlap-add
        for i in 0..self.overlap_buffer.len().min(self.output_buffer.len()) {
            self.output_buffer[i] += self.overlap_buffer[i];
        }
        debug!("Overlap-add completed");

        // Save tail
        let tail_start = self.block_size;
        let tail_end = (tail_start + self.overlap_buffer.len()).min(self.output_buffer.len());
        if tail_end > tail_start {
            self.overlap_buffer[..tail_end - tail_start]
                .copy_from_slice(&self.output_buffer[tail_start..tail_end]);
        }
        debug!("Tail save completed");

        debug!("=== BLOCK END ===");
    }
}

impl Stage for CabinetStage {
    fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // Input sanity check
        if !input.is_finite() {
            debug!("Non-finite input: {}", input);
            return 0.0;
        }

        // Buffer bounds check
        if self.buffer_pos >= self.input_buffer.len() {
            debug!(
                "Buffer overrun! buffer_pos: {}, len: {}",
                self.buffer_pos,
                self.input_buffer.len()
            );
            self.buffer_pos = 0;
            return input * 0.5;
        }

        // Store input
        self.input_buffer[self.buffer_pos] = input;

        // Get output
        let out = if self.buffer_pos < self.output_buffer.len() {
            self.output_buffer[self.buffer_pos]
        } else {
            debug!(
                "Output overrun! buffer_pos: {}, len: {}",
                self.buffer_pos,
                self.output_buffer.len()
            );
            input * 0.5
        };

        self.buffer_pos += 1;

        // Process block when full
        if self.buffer_pos >= self.block_size {
            debug!("Starting block processing...");
            let start = std::time::Instant::now();

            self.process_block();

            let elapsed = start.elapsed();
            debug!("Block processing took: {:?}", elapsed);

            if elapsed.as_millis() > 50 {
                debug!("WARNING: Block processing too slow!");
            }

            self.buffer_pos = 0;
        }

        // Final output check
        if out.is_finite() {
            out
        } else {
            debug!("Non-finite output: {}", out);
            0.0
        }
    }
    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "enabled" => {
                self.enabled = value > 0.5;
                Ok(())
            }
            _ => Err("Unknown parameter name"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "enabled" => Ok(if self.enabled { 1.0 } else { 0.0 }),
            _ => Err("Unknown parameter name"),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
