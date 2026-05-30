use anyhow::{Context, Result};
use log::debug;
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};

const CHANNELS: usize = 1;

pub struct Samplers {
    upsampler: Fft<f32>,
    downsampler: Fft<f32>,
    input_buffer: Vec<Vec<f32>>,
    upsampled_buffer: Vec<Vec<f32>>,
    downsampled_buffer: Vec<Vec<f32>>,
    /// Number of frames the last `upsample()` call actually produced. The
    /// chain processes exactly this many frames in place, so `downsample()`
    /// must feed back exactly this many — not the full buffer capacity.
    upsampled_frames: usize,
    oversample_factor: f64,
    sample_rate: usize,
}

impl Samplers {
    pub fn new(buffer_size: usize, oversample_factor: f64, sample_rate: usize) -> Result<Self> {
        let upsampler = Fft::<f32>::new(
            sample_rate,
            sample_rate * oversample_factor as usize,
            buffer_size,
            1,
            CHANNELS,
            FixedSync::Both,
        )
        .context("failed to create upsampler")?;

        let downsampler = Fft::<f32>::new(
            sample_rate * oversample_factor as usize,
            sample_rate,
            buffer_size * oversample_factor as usize,
            1,
            CHANNELS,
            FixedSync::Both,
        )
        .context("failed to create downsampler")?;

        let mut input_vec = Vec::with_capacity(buffer_size);
        input_vec.resize(buffer_size, 0.0);
        let input_buffer = vec![input_vec];
        let upsampled_buffer = vec![vec![0.0; upsampler.output_frames_max()]; CHANNELS];
        let downsampled_buffer = vec![vec![0.0; downsampler.output_frames_max()]; CHANNELS];
        let upsampled_frames = upsampled_buffer[0].len();

        Ok(Self {
            upsampler,
            downsampler,
            input_buffer,
            upsampled_buffer,
            downsampled_buffer,
            upsampled_frames,
            oversample_factor,
            sample_rate,
        })
    }

    pub const fn get_oversample_factor(&self) -> f64 {
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
        let in_frames = self.input_buffer[0].len();
        let out_frames = self.upsampled_buffer[0].len();

        let input = SequentialSliceOfVecs::new(&self.input_buffer, CHANNELS, in_frames)
            .map_err(|e| anyhow::anyhow!("upsampler input adapter: {e:?}"))?;
        let mut output =
            SequentialSliceOfVecs::new_mut(&mut self.upsampled_buffer, CHANNELS, out_frames)
                .map_err(|e| anyhow::anyhow!("upsampler output adapter: {e:?}"))?;

        let (_, upsampled_frames) = self
            .upsampler
            .process_into_buffer(&input, &mut output, None)
            .context("Upsampler failed")?;
        self.upsampled_frames = upsampled_frames;

        Ok(&mut self.upsampled_buffer[0][..upsampled_frames])
    }

    pub fn downsample(&mut self) -> Result<&mut [f32]> {
        // Feed back exactly the frames the chain just processed in place,
        // not the full buffer capacity, so no stale tail can leak through.
        let in_frames = self.upsampled_frames;
        let out_frames = self.downsampled_buffer[0].len();

        let input = SequentialSliceOfVecs::new(&self.upsampled_buffer, CHANNELS, in_frames)
            .map_err(|e| anyhow::anyhow!("downsampler input adapter: {e:?}"))?;
        let mut output =
            SequentialSliceOfVecs::new_mut(&mut self.downsampled_buffer, CHANNELS, out_frames)
                .map_err(|e| anyhow::anyhow!("downsampler output adapter: {e:?}"))?;

        let (_, downsampled_frames) = self
            .downsampler
            .process_into_buffer(&input, &mut output, None)
            .context("Downsampler failed")?;

        Ok(&mut self.downsampled_buffer[0][..downsampled_frames])
    }

    pub fn resize_buffers(&mut self, new_size: usize) -> Result<()> {
        if self.input_buffer[0].len() == new_size {
            return Ok(());
        }

        debug!(
            "Resizing buffers from {} to {}",
            self.input_buffer[0].len(),
            new_size
        );

        self.input_buffer[0].resize(new_size, 0.0);

        self.upsampler = Fft::<f32>::new(
            self.sample_rate,
            self.sample_rate * self.oversample_factor as usize,
            new_size,
            1,
            CHANNELS,
            FixedSync::Both,
        )
        .context("failed to recreate upsampler")?;
        self.upsampled_buffer = vec![vec![0.0; self.upsampler.output_frames_max()]; CHANNELS];
        self.upsampled_frames = self.upsampled_buffer[0].len();

        self.downsampler = Fft::<f32>::new(
            self.sample_rate * self.oversample_factor as usize,
            self.sample_rate,
            new_size * self.oversample_factor as usize,
            1,
            CHANNELS,
            FixedSync::Both,
        )
        .context("failed to recreate downsampler")?;
        self.downsampled_buffer = vec![vec![0.0; self.downsampler.output_frames_max()]; CHANNELS];

        Ok(())
    }
}
