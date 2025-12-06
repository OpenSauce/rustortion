use anyhow::{Context, Result};
use log::debug;
use rubato::{FftFixedInOut, Resampler};

const CHANNELS: usize = 1;
const SAMPLE_RATE: usize = 48000;

pub struct Samplers {
    upsampler: FftFixedInOut<f32>,
    downsampler: FftFixedInOut<f32>,
    input_buffer: Vec<Vec<f32>>,
    upsampled_buffer: Vec<Vec<f32>>,
    downsampled_buffer: Vec<Vec<f32>>,
    oversample_factor: f64,
}

impl Samplers {
    pub fn new(buffer_size: usize, oversample_factor: f64) -> Result<Self> {
        let upsampler = FftFixedInOut::new(
            SAMPLE_RATE,
            SAMPLE_RATE * oversample_factor as usize,
            buffer_size,
            CHANNELS,
        )
        .unwrap();

        let downsampler = FftFixedInOut::new(
            SAMPLE_RATE * oversample_factor as usize,
            SAMPLE_RATE,
            buffer_size,
            CHANNELS,
        )
        .unwrap();

        let mut input_vec = Vec::with_capacity(buffer_size);
        input_vec.resize(buffer_size, 0.0);
        let input_buffer = vec![input_vec];
        let upsampled_buffer = upsampler.output_buffer_allocate(true);
        let downsampled_buffer = downsampler.output_buffer_allocate(true);

        Ok(Self {
            upsampler,
            downsampler,
            input_buffer,
            upsampled_buffer,
            downsampled_buffer,
            oversample_factor,
        })
    }

    pub fn get_oversample_factor(&self) -> f64 {
        self.oversample_factor
    }

    pub fn copy_input(&mut self, input: &[f32]) -> Result<()> {
        if input.len() != self.input_buffer[0].len() {
            return Err(anyhow::anyhow!(
                "input buffer size mismatch: expected {}, got {}",
                self.input_buffer[0].len(),
                input.len()
            ));
        }
        self.input_buffer[0].copy_from_slice(input);

        Ok(())
    }

    pub fn upsample(&mut self) -> Result<&mut [f32]> {
        let (_, upsampled_frames) = self
            .upsampler
            .process_into_buffer(&self.input_buffer, &mut self.upsampled_buffer, None)
            .context("Upsampler failed")?;

        Ok(&mut self.upsampled_buffer[0][..upsampled_frames])
    }

    pub fn downsample(&mut self) -> Result<&mut [f32]> {
        let (_, downsampled_frames) = self
            .downsampler
            .process_into_buffer(&self.upsampled_buffer, &mut self.downsampled_buffer, None)
            .context("Downsampler failed")?;

        Ok(&mut self.downsampled_buffer[0][..downsampled_frames])
    }

    pub fn downsampled_buffer(&self) -> &[f32] {
        &self.downsampled_buffer[0]
    }

    pub fn resize_buffers(&mut self, new_size: usize) -> Result<()> {
        let upsampler = FftFixedInOut::new(
            SAMPLE_RATE,
            SAMPLE_RATE * self.oversample_factor as usize,
            new_size,
            CHANNELS,
        )
        .unwrap();

        let downsampler = FftFixedInOut::new(
            SAMPLE_RATE * self.oversample_factor as usize,
            SAMPLE_RATE,
            new_size,
            CHANNELS,
        )
        .unwrap();

        self.upsampler = upsampler;
        self.downsampler = downsampler;

        debug!(
            "Upsampler and downsampler resized to { } frames",
            self.upsampler.input_frames_max()
        );

        Ok(())
    }
}
