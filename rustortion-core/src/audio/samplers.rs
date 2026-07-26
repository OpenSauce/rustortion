use anyhow::{Context, Result};
use log::debug;
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler, WindowFunction};

const CHANNELS: usize = 1;

/// Input/output FIFOs used by [`Samplers`] in variable-block mode.
///
/// `rubato`'s `Fft` resampler is a *fixed* chunk-size resampler: it always
/// consumes `chunk_size_in` frames and produces `chunk_size_out` frames, and it
/// is neither resizable (`as_resizable()` returns `None`) nor able to accept a
/// short chunk without zero-filling the remainder (`Indexing::partial_len`
/// literally writes zeros into the missing frames, which is only correct for
/// the final chunk of a finite clip). Hosts, however, hand out blocks of any
/// length up to the maximum, so the fixed chunk has to be decoupled from the
/// host block size.
///
/// The FIFOs do exactly that: incoming frames are accumulated until a full
/// chunk is available, whole chunks are pushed through the resampler + amp
/// chain, and the host block is served from the produced output. The output
/// FIFO is pre-filled with `prefill` frames of silence so a read can never
/// underrun — that pre-fill *is* the added latency, which the plugin reports to
/// the host for delay compensation.
///
/// Because that latency is not free, the FIFO starts **disengaged**. Blocks
/// whose length is an exact multiple of `chunk` are processed straight through
/// (see `Engine::process_with_upsampling`), which is every block in a
/// fixed-block host: 64, 128, 256, 512, 1024 and 2048 are all multiples of the
/// 64-frame chunk. The first block that is not engages the FIFO, for
/// good — a session that never sees one never pays the prefill. The buffers are
/// allocated up front either way, so engaging is a flag flip plus a `fill(0.0)`
/// of memory already owned, never an allocation.
///
/// `prefill = max_block + chunk` is both necessary and sufficient. The total
/// number of frames held across the two FIFOs is invariant (each call adds `n`
/// on the input side and removes `n` on the output side, and resampling moves
/// frames 1:1 between them), so it stays at `prefill`. After processing, the
/// input side holds `a < chunk` leftover frames, leaving `prefill - a` on the
/// output side; requiring that to cover the largest possible read gives
/// `prefill >= max_block + chunk - 1`.
///
/// Both FIFOs are drained by compaction (`copy_within`), never reallocated, so
/// nothing here allocates on the audio thread.
struct BlockFifo {
    input: Vec<f32>,
    input_len: usize,
    output: Vec<f32>,
    output_len: usize,
    /// Frames of silence pre-loaded into the output FIFO == added latency.
    prefill: usize,
    /// Largest block the caller may push, remembered so the FIFOs can be
    /// rebuilt if the resampler chunk changes.
    max_block: usize,
    /// Whether the FIFO is actually in use. Sticky: once engaged it never
    /// disengages, because every transition re-reports latency to the host and
    /// each of those can restart playback.
    engaged: bool,
}

impl BlockFifo {
    /// `max_block` is the largest block the caller may ever push; `chunk` is the
    /// fixed resampler chunk size.
    fn new(max_block: usize, chunk: usize) -> Self {
        let prefill = max_block + chunk;
        Self {
            // Up to `chunk - 1` leftover frames plus one whole incoming block.
            input: vec![0.0; max_block + chunk],
            input_len: 0,
            // `prefill`, plus everything a single call can add before the read.
            output: vec![0.0; 2 * (max_block + chunk)],
            // Empty until engaged; the prefill is written in `engage`.
            output_len: 0,
            prefill,
            max_block,
            engaged: false,
        }
    }

    /// Start buffering. Allocation-free by construction: both `Vec`s were sized
    /// in `new` and are only written to here.
    fn engage(&mut self) {
        self.engaged = true;
        self.reset();
    }

    /// Drop every queued frame, leaving the FIFO as it was the moment it
    /// engaged (or, if it never engaged, empty).
    ///
    /// Deliberately does **not** change `engaged`. That flag is sticky because
    /// every transition re-reports latency to the host, and reset runs on the
    /// audio thread where no latency can be re-reported — so an engaged FIFO is
    /// re-primed with its full pre-fill of silence rather than emptied, keeping
    /// [`Samplers::latency_frames`] invariant across the reset.
    ///
    /// Only lengths and the pre-fill region are touched; frames past
    /// `input_len` / `output_len` are never read. Allocation-free.
    fn reset(&mut self) {
        self.input_len = 0;
        if self.engaged {
            self.output[..self.prefill].fill(0.0);
            self.output_len = self.prefill;
        } else {
            self.output_len = 0;
        }
    }
}

/// Resampler chunk size used in variable-block mode, in frames at the base
/// sample rate. Decoupled from the host's maximum block size.
///
/// The fast path requires the host block to be an exact *multiple* of this, so
/// the value has to divide every block size a host might use — and hosts
/// routinely run blocks far smaller than the `max_buffer_size` they declare.
/// 64 is the largest value that divides all of 64, 128, 256, 512, 1024 and
/// 2048, so every standard buffer size stays on the fast path even when the
/// declared maximum is much larger. Anything bigger (128, 256) would force a
/// 64-frame low-latency session — precisely the session that can least afford
/// it — onto the buffered path forever.
///
/// It also sets both latency figures: the resamplers' group delay is
/// proportional to it, and the engaged pre-fill is `max_block + chunk`.
///
/// The cost is a shorter anti-aliasing filter (the FFT block is `chunk` frames,
/// versus the ~256 `rubato` suggests as a quality/delay compromise): the
/// transition band around Nyquist widens to roughly 3 kHz at 48 kHz, so the
/// top of the audible band rolls off slightly earlier and image rejection just
/// below the stopband is less complete. For an amp simulator — where
/// oversampling exists to keep the *nonlinear* stages from folding harmonics
/// back down, not to deliver mastering-grade rate conversion — that is a good
/// trade for 4x less latency and universal fast-path coverage.
const VARIABLE_BLOCK_CHUNK: usize = 64;

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
    /// `Some` when the caller may hand in blocks shorter than the constructed
    /// chunk size (plugin hosts). `None` for the fixed-period JACK path, which
    /// resizes the samplers to match the period and therefore needs neither the
    /// FIFOs nor the latency they add.
    fifo: Option<BlockFifo>,
}

impl Samplers {
    pub fn new(buffer_size: usize, oversample_factor: f64, sample_rate: usize) -> Result<Self> {
        let upsampler = Fft::<f32>::new_custom(
            sample_rate,
            sample_rate * oversample_factor as usize,
            buffer_size,
            1,
            CHANNELS,
            WindowFunction::BlackmanHarris2,
            FixedSync::Both,
        )
        .context("failed to create upsampler")?;

        let downsampler = Fft::<f32>::new_custom(
            sample_rate * oversample_factor as usize,
            sample_rate,
            buffer_size * oversample_factor as usize,
            1,
            CHANNELS,
            WindowFunction::BlackmanHarris2,
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
            fifo: None,
        })
    }

    /// Same as [`Samplers::new`], but tolerant of blocks shorter than
    /// `buffer_size`. See [`BlockFifo`] for why this is needed and what it
    /// costs. `buffer_size` must be the *maximum* block the caller can deliver.
    pub fn new_variable_block(
        max_block: usize,
        oversample_factor: f64,
        sample_rate: usize,
    ) -> Result<Self> {
        let chunk = VARIABLE_BLOCK_CHUNK.min(max_block.max(1));
        let mut samplers = Self::new(chunk, oversample_factor, sample_rate)?;
        samplers.fifo = Some(BlockFifo::new(max_block, chunk));
        Ok(samplers)
    }

    pub const fn get_oversample_factor(&self) -> f64 {
        self.oversample_factor
    }

    /// True when this instance tolerates blocks that are not exactly one chunk
    /// long (plugin hosts), whether or not the FIFO has engaged yet.
    pub const fn is_variable_block(&self) -> bool {
        self.fifo.is_some()
    }

    /// The fixed number of frames the resamplers consume and produce per call.
    pub fn chunk_frames(&self) -> usize {
        self.input_buffer[0].len()
    }

    /// Whether the FIFO has engaged. While this is false the caller must feed
    /// whole chunks and gets zero buffering latency.
    pub fn fifo_engaged(&self) -> bool {
        self.fifo.as_ref().is_some_and(|f| f.engaged)
    }

    /// Engage the FIFO permanently. RT-safe: no allocation, just a fill of
    /// already-owned memory. Idempotent, but engaging twice would re-insert the
    /// prefill, so callers should check [`Samplers::fifo_engaged`] first.
    pub fn engage_fifo(&mut self) {
        if let Some(fifo) = self.fifo.as_mut()
            && !fifo.engaged
        {
            fifo.engage();
        }
    }

    /// Latency, in frames at the base (host) sample rate, that this instance
    /// adds to the signal path *in its current mode*: the two resamplers' own
    /// group delay, plus the FIFO pre-fill only once the FIFO has engaged. Zero
    /// when not oversampling, since the resamplers are then bypassed entirely
    /// by the engine.
    pub fn latency_frames(&self) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let factor = self.oversample_factor as usize;
        if factor <= 1 {
            return 0;
        }
        // `output_delay()` is expressed in output frames: the upsampler's
        // output runs at the oversampled rate, the downsampler's at the base
        // rate.
        let resampler_delay =
            self.upsampler.output_delay() / factor + self.downsampler.output_delay();
        let fifo_delay = self
            .fifo
            .as_ref()
            .filter(|f| f.engaged)
            .map_or(0, |f| f.prefill);
        resampler_delay + fifo_delay
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

    /// Append a host block to the input FIFO.
    ///
    /// Frames that do not fit are dropped rather than panicking. This is only
    /// reachable if the caller exceeds the maximum block size it declared, and
    /// it degrades badly when it happens — a steady stream of oversized blocks
    /// loses most of each one, because the FIFO drains at the declared rate.
    /// The plugin therefore clamps and silences oversized blocks before they
    /// reach here; this is the last line of defence, not the intended path.
    pub fn fifo_push_input(&mut self, input: &[f32]) {
        let Some(fifo) = self.fifo.as_mut() else {
            return;
        };
        let room = fifo.input.len() - fifo.input_len;
        let n = input.len().min(room);
        fifo.input[fifo.input_len..fifo.input_len + n].copy_from_slice(&input[..n]);
        fifo.input_len += n;
    }

    /// Move one full chunk from the input FIFO into the resampler input buffer.
    /// Returns `false` when fewer than `chunk` frames are queued, which ends the
    /// caller's processing loop for this block.
    pub fn fifo_load_chunk(&mut self) -> bool {
        let chunk = self.input_buffer[0].len();
        let Some(fifo) = self.fifo.as_mut() else {
            return false;
        };
        if fifo.input_len < chunk {
            return false;
        }
        self.input_buffer[0].copy_from_slice(&fifo.input[..chunk]);
        fifo.input.copy_within(chunk..fifo.input_len, 0);
        fifo.input_len -= chunk;
        true
    }

    /// Downsample the chain's output and append it to the output FIFO.
    ///
    /// Same work as [`Samplers::downsample`], but the result is queued instead
    /// of being handed back, so the caller does not have to hold a borrow of
    /// the sampler while it writes into the FIFO.
    pub fn downsample_into_fifo(&mut self) -> Result<()> {
        let produced = self.downsample()?.len();
        let Some(fifo) = self.fifo.as_mut() else {
            return Ok(());
        };
        let room = fifo.output.len() - fifo.output_len;
        let n = produced.min(room);
        fifo.output[fifo.output_len..fifo.output_len + n]
            .copy_from_slice(&self.downsampled_buffer[0][..n]);
        fifo.output_len += n;
        Ok(())
    }

    /// Serve a host block from the output FIFO. Any shortfall (which the
    /// pre-fill is sized to prevent) is filled with silence rather than stale
    /// samples.
    pub fn fifo_pop_output(&mut self, output: &mut [f32]) {
        let Some(fifo) = self.fifo.as_mut() else {
            output.fill(0.0);
            return;
        };
        let n = output.len().min(fifo.output_len);
        output[..n].copy_from_slice(&fifo.output[..n]);
        output[n..].fill(0.0);
        fifo.output.copy_within(n..fifo.output_len, 0);
        fifo.output_len -= n;
    }

    /// Drop all in-flight audio: the two resamplers' overlap/scratch state, the
    /// staging buffers, and anything queued in the variable-block FIFO.
    ///
    /// RT-safe. `rubato`'s `Fft::reset` only zero-fills buffers it already owns
    /// (and `update_chunk_sizes` is a no-op for `FixedSync::Both`, which is how
    /// both resamplers here are built), and the FIFO reset is lengths plus a
    /// fill of memory allocated in `new`.
    ///
    /// The reported latency is unchanged by design — see [`BlockFifo::reset`].
    pub fn reset(&mut self) {
        self.upsampler.reset();
        self.downsampler.reset();

        self.input_buffer[0].fill(0.0);
        self.upsampled_buffer[0].fill(0.0);
        self.downsampled_buffer[0].fill(0.0);
        // Matches `new`: until the next `upsample()` sets it, treat the whole
        // (now silent) buffer as valid so a stray `downsample()` reads zeros
        // rather than a stale frame count into stale audio.
        self.upsampled_frames = self.upsampled_buffer[0].len();

        if let Some(fifo) = self.fifo.as_mut() {
            fifo.reset();
        }
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

        self.upsampler = Fft::<f32>::new_custom(
            self.sample_rate,
            self.sample_rate * self.oversample_factor as usize,
            new_size,
            1,
            CHANNELS,
            WindowFunction::BlackmanHarris2,
            FixedSync::Both,
        )
        .context("failed to recreate upsampler")?;
        self.upsampled_buffer = vec![vec![0.0; self.upsampler.output_frames_max()]; CHANNELS];
        self.upsampled_frames = self.upsampled_buffer[0].len();

        self.downsampler = Fft::<f32>::new_custom(
            self.sample_rate * self.oversample_factor as usize,
            self.sample_rate,
            new_size * self.oversample_factor as usize,
            1,
            CHANNELS,
            WindowFunction::BlackmanHarris2,
            FixedSync::Both,
        )
        .context("failed to recreate downsampler")?;
        self.downsampled_buffer = vec![vec![0.0; self.downsampler.output_frames_max()]; CHANNELS];

        if let Some(max_block) = self.fifo.as_ref().map(|f| f.max_block) {
            self.fifo = Some(BlockFifo::new(max_block, new_size));
        }

        Ok(())
    }
}
