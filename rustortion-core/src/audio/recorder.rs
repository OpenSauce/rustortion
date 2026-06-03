use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, TrySendError, bounded};
use hound::WavWriter;
use log::{error, info};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, thread};

type AudioBlock = Vec<i16>;

/// Pre-allocate enough buffering for this many seconds of audio. Bounded (so the
/// RT thread never allocates), but sized so large it's effectively unbounded in
/// practice: the writer would have to fall this far behind — a multi-second disk
/// stall — before `record_block` drops anything. At stereo 16-bit this is only
/// ~`BUFFER_SECONDS * sample_rate * 4` bytes (≈1.5 MB for 8 s @ 48 kHz).
const BUFFER_SECONDS: usize = 8;
/// Floor on the buffer size in blocks, in case the host block size is huge.
const MIN_BUFFER_BLOCKS: usize = 16;

pub struct Recorder {
    /// Non-blocking handoff of filled buffers to the writer thread.
    recorder_sender: Sender<AudioBlock>,
    /// Pool of emptied buffers returned by the writer thread for reuse, so
    /// `record_block` never allocates on the RT thread.
    recycle_receiver: Receiver<AudioBlock>,
    /// Used to return a buffer to the pool when the handoff channel is full.
    recycle_sender: Sender<AudioBlock>,
    /// Largest input block (in samples) the pre-allocated buffers can hold
    /// without reallocating. Blocks larger than this are dropped.
    max_block_samples: usize,
    /// Count of blocks dropped because the writer couldn't keep up (disk
    /// stall). The RT thread never blocks on the writer — it drops instead —
    /// so this surfaces any lost audio.
    overruns: Arc<AtomicU64>,
    handle: thread::JoinHandle<()>,
}

impl Recorder {
    /// Creates a new Recorder instance.
    ///
    /// `max_block_samples` is the largest input block size the recorder will be
    /// asked to handle; the buffer pool is pre-sized to it so that
    /// `record_block` performs no allocation on the RT thread.
    pub fn new(sample_rate: u32, record_dir: &str, max_block_samples: usize) -> Result<Self> {
        // Size the buffer pool / handoff channel by time so it absorbs several
        // seconds of writer lag before ever dropping a block. Both the channel
        // and the pool hold the same number of buffers so the producer never
        // starves the pool while the channel still has room.
        let buffer_blocks = (BUFFER_SECONDS * sample_rate as usize)
            .div_ceil(max_block_samples.max(1))
            .max(MIN_BUFFER_BLOCKS);

        let (recorder_sender, recorder_receiver) = bounded::<AudioBlock>(buffer_blocks);
        let (recycle_sender, recycle_receiver) = bounded::<AudioBlock>(buffer_blocks);
        fs::create_dir_all(record_dir)?;

        // Pre-allocate the buffer pool. Each input sample becomes two
        // interleaved stereo `i16`s, so size for `max_block_samples * 2`.
        for _ in 0..buffer_blocks {
            // Can't fail: the channel is empty and sized to match the loop.
            let _ = recycle_sender.try_send(AudioBlock::with_capacity(max_block_samples * 2));
        }

        let filename = format!(
            "{record_dir}/recording_{}.wav",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        info!("Recording to: {filename}");

        let writer_recycle_sender = recycle_sender.clone();
        let handle = thread::spawn(move || {
            run_writer_thread(
                sample_rate,
                filename,
                recorder_receiver,
                &writer_recycle_sender,
            );
        });

        Ok(Self {
            recorder_sender,
            recycle_receiver,
            recycle_sender,
            max_block_samples,
            overruns: Arc::new(AtomicU64::new(0)),
            handle,
        })
    }

    /// Number of audio blocks dropped because the writer thread fell behind.
    /// Zero in normal operation; non-zero indicates the disk couldn't keep up.
    pub fn overruns(&self) -> u64 {
        self.overruns.load(Ordering::Relaxed)
    }

    /// Stops the recording and waits for the writer thread to finish.
    /// This is needed for WAV files to be finalized properly.
    pub fn stop(self) -> Result<()> {
        drop(self.recorder_sender);
        self.handle
            .join()
            .map_err(|e| anyhow::anyhow!("Writer thread panicked (join failed): {e:?}"))
    }

    /// Record a block of `f32` samples by handing a filled buffer to the writer
    /// thread.
    ///
    /// Real-time safe: it never allocates and never blocks. It takes a
    /// pre-allocated buffer from the recycle pool, fills it, and `try_send`s it
    /// to the writer (which buffers in memory and flushes to disk on its own
    /// thread). If the writer has fallen behind — pool empty or handoff channel
    /// full — the block is dropped and an overrun is recorded rather than
    /// stalling the audio thread on disk I/O.
    pub fn record_block(&self, samples: &[f32]) -> Result<()> {
        if samples.len() > self.max_block_samples {
            self.overruns.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let Ok(mut block) = self.recycle_receiver.try_recv() else {
            self.overruns.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        };
        block.clear();
        for sample in samples {
            let v = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            block.push(v);
            block.push(v);
        }
        match self.recorder_sender.try_send(block) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(block)) => {
                // Writer behind: return the buffer to the pool, drop the audio.
                let _ = self.recycle_sender.try_send(block);
                self.overruns.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(TrySendError::Disconnected(_)) => {
                Err(anyhow::anyhow!("recorder writer thread has stopped"))
            }
        }
    }
}

/// Runs the writer thread, that writes audio blocks received over its channel to a WAV file.
fn run_writer_thread(
    sample_rate: u32,
    filename: String,
    recorder_receiver: Receiver<AudioBlock>,
    recycle_sender: &Sender<AudioBlock>,
) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = match WavWriter::create(&filename, spec) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create WAV file '{filename}': {e}");
            return;
        }
    };

    for block in recorder_receiver {
        for &sample in &block {
            if let Err(e) = writer.write_sample(sample) {
                error!("Failed to write sample to WAV file '{filename}': {e}");
            }
        }
        // Return the buffer to the pool for reuse. If the pool is full or the
        // RT side has gone away, just drop it.
        let _ = recycle_sender.try_send(block);
    }

    if let Err(e) = writer.finalize() {
        error!("Failed to finalize WAV file: {e}");
    } else {
        info!("Recording saved: {filename}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;
    use std::f32::consts::PI;
    use tempfile::TempDir;

    #[test]
    fn test_recorder() -> Result<()> {
        const SAMPLE_RATE: u32 = 48000;
        const TEST_FREQ: f32 = 440.0;
        const DURATION_SECS: f32 = 3.0;
        const AMPLITUDE: f32 = 0.5;

        let temp_dir = TempDir::new()?;
        let record_dir = temp_dir.path().to_str().unwrap();

        let block_size = 256;
        let recorder = Recorder::new(SAMPLE_RATE, record_dir, block_size)?;

        let total_samples = (SAMPLE_RATE as f32 * DURATION_SECS) as usize;
        let mut generated_samples = 0;

        while generated_samples < total_samples {
            let samples_to_generate = (total_samples - generated_samples).min(block_size);
            let mut block = Vec::with_capacity(samples_to_generate);

            for i in 0..samples_to_generate {
                let sample_idx = generated_samples + i;
                let t = sample_idx as f32 / SAMPLE_RATE as f32;
                let sample = (2.0 * PI * TEST_FREQ * t).sin() * AMPLITUDE;
                block.push(sample);
            }

            recorder.record_block(&block)?;
            generated_samples += samples_to_generate;
        }

        recorder.stop()?;

        let entries = std::fs::read_dir(record_dir)?;
        let wav_file = entries
            .filter_map(std::result::Result::ok)
            .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("wav"))
            .expect("No WAV file found");

        let wav_path = wav_file.path();
        assert!(wav_path.exists(), "WAV file does not exist");

        let mut reader = WavReader::open(&wav_path)?;
        let spec = reader.spec();

        assert_eq!(spec.sample_rate, SAMPLE_RATE, "Sample rate mismatch");
        assert_eq!(spec.channels, 2, "Channel count mismatch");
        assert_eq!(spec.bits_per_sample, 16, "Bit depth mismatch");

        let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?;
        let recorded_samples = samples.len() / 2;

        let expected_samples = total_samples;
        let sample_diff = (recorded_samples as i32 - expected_samples as i32).abs();
        let tolerance = block_size * 2;

        assert!(
            sample_diff < tolerance as i32,
            "Duration mismatch: {sample_diff} samples (expected within {tolerance})"
        );

        let mut mono_samples = Vec::with_capacity(recorded_samples);
        for i in (0..samples.len()).step_by(2) {
            mono_samples.push(samples[i] as f32 / i16::MAX as f32);
        }

        let mut zero_crossings = 0;
        let mut last_sign = mono_samples[0] >= 0.0;

        for &sample in &mono_samples {
            let current_sign = sample >= 0.0;
            if current_sign != last_sign {
                zero_crossings += 1;
            }
            last_sign = current_sign;
        }

        let cycles = zero_crossings as f32 / 2.0;
        let duration = mono_samples.len() as f32 / SAMPLE_RATE as f32;
        let measured_freq = cycles / duration;

        let freq_error_percent = ((measured_freq - TEST_FREQ) / TEST_FREQ * 100.0).abs();
        assert!(
            freq_error_percent < 0.5,
            "Frequency error too large: {freq_error_percent:.4}%"
        );

        let num_segments = 10;
        let segment_size = mono_samples.len() / num_segments;
        let mut segment_freqs = Vec::new();

        for seg in 0..num_segments {
            let start = seg * segment_size;
            let end = (start + segment_size).min(mono_samples.len());
            let segment = &mono_samples[start..end];

            let mut zc = 0;
            let mut last = segment[0] >= 0.0;
            for &s in segment {
                let curr = s >= 0.0;
                if curr != last {
                    zc += 1;
                }
                last = curr;
            }

            let seg_duration = segment.len() as f32 / SAMPLE_RATE as f32;
            let seg_freq = (zc as f32 / 2.0) / seg_duration;
            segment_freqs.push(seg_freq);
        }

        let first_freq = segment_freqs[0];
        let last_freq = segment_freqs[num_segments - 1];
        let drift_hz = last_freq - first_freq;
        let drift_percent = (drift_hz / TEST_FREQ * 100.0).abs();

        assert!(
            drift_percent < 0.2,
            "Timing drift detected: {drift_percent:.4}% (sample rate issue?)"
        );

        let mut channel_diff = 0u64;
        for i in (0..samples.len() - 1).step_by(2) {
            channel_diff += (samples[i] - samples[i + 1]).unsigned_abs() as u64;
        }

        assert_eq!(channel_diff, 0, "Stereo channels are not identical");

        let max_sample = mono_samples.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        let expected_max = AMPLITUDE;
        let amplitude_error = (max_sample - expected_max).abs() / expected_max;

        assert!(
            amplitude_error < 0.02,
            "Amplitude error too large: {:.2}%",
            amplitude_error * 100.0
        );

        Ok(())
    }
}
