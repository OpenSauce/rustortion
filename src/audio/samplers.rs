use anyhow::{Context, Result};
use log::info;
use rubato::{FftFixedInOut, Resampler};

const CHANNELS: usize = 1;

pub struct Samplers {
    upsampler: FftFixedInOut<f32>,
    downsampler: FftFixedInOut<f32>,
    input_buffer: Vec<Vec<f32>>,
    upsampled_buffer: Vec<Vec<f32>>,
    downsampled_buffer: Vec<Vec<f32>>,
    oversample_factor: f64,
    sample_rate: usize,
}

impl Samplers {
    pub fn new(buffer_size: usize, oversample_factor: f64, sample_rate: usize) -> Result<Self> {
        let upsampler = FftFixedInOut::new(
            sample_rate,
            sample_rate * oversample_factor as usize,
            buffer_size,
            CHANNELS,
        )
        .context("failed to create upsampler")?;

        let downsampler = FftFixedInOut::new(
            sample_rate * oversample_factor as usize,
            sample_rate,
            buffer_size * oversample_factor as usize,
            CHANNELS,
        )
        .context("failed to create downsampler")?;

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
            sample_rate,
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
        if self.input_buffer[0].len() == new_size {
            return Ok(());
        }

        info!(
            "Resizing buffers from {} to {}",
            self.input_buffer[0].len(),
            new_size
        );

        self.input_buffer[0].resize(new_size, 0.0);

        self.upsampler = FftFixedInOut::new(
            self.sample_rate,
            self.sample_rate * self.oversample_factor as usize,
            new_size,
            CHANNELS,
        )
        .context("failed to recreate upsampler")?;
        self.upsampled_buffer = self.upsampler.output_buffer_allocate(true);

        self.downsampler = FftFixedInOut::new(
            self.sample_rate * self.oversample_factor as usize,
            self.sample_rate,
            new_size * self.oversample_factor as usize,
            CHANNELS,
        )
        .context("failed to recreate downsampler")?;
        self.downsampled_buffer = self.downsampler.output_buffer_allocate(true);

        Ok(())
    }
}
