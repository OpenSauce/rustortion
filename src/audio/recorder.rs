use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use hound::WavWriter;
use log::{error, info};
use std::{fs, thread};

pub type AudioBlock = Vec<i16>;
const BLOCK_CHANNEL_CAPACITY: usize = 32;

pub struct Recorder {
    tx: Sender<AudioBlock>,
    handle: thread::JoinHandle<()>,
}

impl Recorder {
    /// Creates a new Recorder instance.
    pub fn new(sample_rate: u32, record_dir: &str) -> Result<Self> {
        let (tx, rx) = bounded::<AudioBlock>(BLOCK_CHANNEL_CAPACITY);
        fs::create_dir_all(record_dir)?;

        let filename = format!(
            "{record_dir}/recording_{}.wav",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        info!("Recording to: {filename}");

        let handle = thread::spawn(move || run_writer_thread(sample_rate, filename, rx));

        Ok(Self { tx, handle })
    }

    /// Returns a clone of the sender for sending audio blocks.
    pub fn sender(&self) -> Sender<AudioBlock> {
        self.tx.clone()
    }

    /// Stops the recording and waits for the writer thread to finish.
    /// This is needed for WAV files to be finalized properly.
    pub fn stop(self) -> Result<()> {
        drop(self.tx);
        self.handle
            .join()
            .map_err(|e| anyhow::anyhow!("Writer thread panicked (join failed): {:?}", e))
    }
}

/// Runs the writer thread, that writes audio blocks received over its channel to a WAV file.
fn run_writer_thread(sample_rate: u32, filename: String, rx: Receiver<AudioBlock>) {
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

    for block in rx {
        for &sample in &block {
            if let Err(e) = writer.write_sample(sample) {
                error!("Failed to write sample to WAV file '{filename}': {e}");
            }
        }
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

        let recorder = Recorder::new(SAMPLE_RATE, record_dir)?;
        let tx = recorder.sender();

        let total_samples = (SAMPLE_RATE as f32 * DURATION_SECS) as usize;
        let block_size = 256;
        let mut generated_samples = 0;

        while generated_samples < total_samples {
            let mut block = Vec::new();

            let samples_to_generate = (total_samples - generated_samples).min(block_size);

            for i in 0..samples_to_generate {
                let sample_idx = generated_samples + i;
                let t = sample_idx as f32 / SAMPLE_RATE as f32;
                let sample = (2.0 * PI * TEST_FREQ * t).sin() * AMPLITUDE;
                let sample_i16 = (sample * i16::MAX as f32) as i16;

                block.push(sample_i16);
                block.push(sample_i16);
            }

            tx.send(block)?;
            generated_samples += samples_to_generate;
        }

        drop(tx);
        recorder.stop()?;

        let entries = std::fs::read_dir(record_dir)?;
        let wav_file = entries
            .filter_map(|e| e.ok())
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
            "Duration mismatch: {} samples (expected within {})",
            sample_diff,
            tolerance
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
            "Frequency error too large: {:.4}%",
            freq_error_percent
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
            "Timing drift detected: {:.4}% (sample rate issue?)",
            drift_percent
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
