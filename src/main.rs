use jack::{Client, ClientOptions};
use serde_json::from_reader;
use std::fs::File;
use std::io::BufReader;
use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

mod amp;
mod processor;
mod recorder;

use amp::{Amp, AmpConfig};
use clap::Parser;
use processor::Processor;
use recorder::Recorder;

#[derive(Parser, Debug)]
#[command(name = "rustortion")]
#[command(author = "OpenSauce")]
#[command(version = "0.1")]
#[command(about = "An amp sim with optional WAV recording.")]
struct Args {
    #[arg(long)]
    recording: bool,

    #[arg(long)]
    preset_path: String,
}

fn main() {
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "128/48000");
        env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
    }

    let args = Args::parse();
    let recording = args.recording;

    let config = load_amp_config(&args.preset_path).expect("Failed to load preset file");

    println!(
        "🔥 Rustortion: {}",
        if recording { "🛑 Recording!" } else { "" }
    );
    println!("{:?}", config);

    let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER).unwrap();
    let sample_rate = client.sample_rate() as f32;
    let amp = Amp::new(config, sample_rate);

    let recorder = if recording {
        Some(Recorder::new(sample_rate as u32, "./recordings").expect("failed to start recorder"))
    } else {
        None
    };
    let tx = recorder.as_ref().map(|r| r.sender());
    let processor = Processor::new(&client, amp, tx);
    let process = processor.into_process_handler();

    let _active_client = client
        .activate_async(Notifications, jack::ClosureProcessHandler::new(process))
        .unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\nCtrl+C received, shutting down...");

        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }
    if let Some(rec) = recorder {
        rec.stop(); // join disk thread
    }
}

fn load_amp_config(path: &str) -> std::io::Result<AmpConfig> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config: AmpConfig =
        from_reader(reader).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(config)
}

struct Notifications;
impl jack::NotificationHandler for Notifications {}
