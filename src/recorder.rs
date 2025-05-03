use crossbeam::channel::{Receiver, Sender, bounded};
use hound::WavWriter;
use std::{fs, thread};

/// One block = inter‑leaved stereo samples (L R L R …).
pub type AudioBlock = Vec<i16>;
pub const BLOCK_FRAMES: usize = 256; // tweak latency here

/// Handle returned to the caller.
pub struct Recorder {
    tx: Sender<AudioBlock>,
    handle: thread::JoinHandle<()>,
}

impl Recorder {
    pub fn new(sample_rate: u32, record_dir: &str) -> std::io::Result<Self> {
        let (tx, rx) = bounded::<AudioBlock>(32); // 32×256 ≈ 5 s @48 kHz
        fs::create_dir_all(record_dir)?;

        let filename = format!(
            "{record_dir}/recording_{}.wav",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        println!("Recording to: {filename}");

        let handle = thread::spawn(move || run_writer_thread(sample_rate, filename, rx));

        Ok(Self { tx, handle })
    }

    pub fn sender(&self) -> Sender<AudioBlock> {
        self.tx.clone()
    }

    pub fn stop(self) {
        drop(self.tx);
        self.handle.join().expect("Unable to join thread");
    }
}

fn run_writer_thread(sample_rate: u32, filename: String, rx: Receiver<AudioBlock>) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(filename, spec).unwrap();

    for block in rx {
        for sample in &block {
            writer.write_sample(*sample).unwrap();
        }
    }

    writer.finalize().expect("Failed to finalise WAV file");
}
