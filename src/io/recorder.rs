use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use hound::WavWriter;
use log::{error, info};
use std::{fs, thread};

pub type AudioBlock = Vec<i16>;
pub const BLOCK_FRAMES: usize = 128;
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

/// Runs the writer thread, that writes audio blocks recieved over its channel to a WAV file.
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
