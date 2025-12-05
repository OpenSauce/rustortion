use anyhow::{Context, Result};
use log::info;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub struct Samplers {
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
    input_buffer: Vec<Vec<f32>>,
    upsampled_buffer: Vec<Vec<f32>>,
    downsampled_buffer: Vec<Vec<f32>>,
    oversample_factor: f64,
}

impl Samplers {
    pub fn new(buffer_size: usize, oversample_factor: f64) -> Result<Self> {
        const CHANNELS: usize = 1;
        const MAX_BLOCK_SIZE: usize = 8192;

        let interp_params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let down_interp_params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        let mut upsampler = SincFixedIn::<f32>::new(
            oversample_factor,
            1.0,
            interp_params,
            MAX_BLOCK_SIZE,
            CHANNELS,
        )
        .context("failed to create upsampler")?;

        let mut downsampler = SincFixedIn::<f32>::new(
            1.0 / oversample_factor,
            1.0,
            down_interp_params,
            MAX_BLOCK_SIZE * oversample_factor as usize,
            CHANNELS,
        )
        .context("failed to create downsampler")?;

        upsampler
            .set_chunk_size(buffer_size)
            .context("failed to set upsampler chunk size")?;
        downsampler
            .set_chunk_size(buffer_size * oversample_factor as usize)
            .context("failed to set downsampler chunk size")?;

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
        let buffer = &mut self.input_buffer[0];
        if buffer.len() != new_size {
            buffer.resize(new_size, 0.0);
            info!("Input buffer resized to {new_size}");
        }

        self.upsampler
            .set_chunk_size(new_size)
            .context("failed to resize upsampler")?;
        self.upsampled_buffer = self.upsampler.output_buffer_allocate(true);

        let downsampler_size = new_size * self.oversample_factor as usize;
        self.downsampler
            .set_chunk_size(downsampler_size)
            .context("failed to resize downsampler")?;
        self.downsampled_buffer = self.downsampler.output_buffer_allocate(true);

        Ok(())
    }
}
