use jack::{Client, ClientOptions};
use std::{
    env,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

mod amp;
mod processor;

use amp::{Amp, DistortionMode};
use clap::Parser;
use processor::Processor;

#[derive(Parser, Debug)]
#[command(name = "rustortion")]
#[command(author = "OpenSauce")]
#[command(version = "0.1")]
#[command(about = "A metal-mode JACK amp sim with optional WAV recording.")]
struct Args {
    #[arg(long, default_value_t = 1.0)]
    gain: f32,

    #[arg(long)]
    recording: bool,

    #[arg(long, env = "RUSTORTION_MODE", default_value_t = DistortionMode::Tanh)]
    mode: DistortionMode,
}

fn main() {
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "64/48000");
        env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
    }

    let args = Args::parse();
    let gain = args.gain;
    let recording = args.recording;

    if recording {
        std::fs::create_dir_all("./recordings").unwrap();
    }

    let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER).unwrap();

    let sample_rate = client.sample_rate() as f32;
    let amp = Arc::new(Mutex::new(Amp::new(gain, sample_rate, args.mode)));
    let (processor, writer) = Processor::new(&client, Arc::clone(&amp), recording);
    let process = processor.into_process_handler();

    let _active_client = client
        .activate_async(Notifications, jack::ClosureProcessHandler::new(process))
        .unwrap();

    println!(
        "🔥 Rustortion: Metal mode active (gain {:.2}, mode: {}){}!",
        gain,
        args.mode,
        if recording { " [🎙 recording]" } else { "" }
    );

    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);
    let writer_clone = writer.clone();

    ctrlc::set_handler(move || {
        println!("\n🛑 Ctrl+C received, shutting down...");

        if let Some(writer_arc) = &writer_clone {
            if let Ok(mut maybe_writer) = writer_arc.lock() {
                if let Some(writer) = maybe_writer.take() {
                    writer.finalize().expect("Failed to finalize WAV file");
                    println!("💾 Recording saved to recording.wav");
                }
            }
        }

        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }
}

struct Notifications;
impl jack::NotificationHandler for Notifications {}
