use jack::{Client, ClientOptions};
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

use processor::Processor;

fn main() {
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "64/48000");
        env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
    }

    let args: Vec<String> = env::args().skip(1).collect();

    let gain: f32 = args
        .iter()
        .find_map(|arg| arg.parse::<f32>().ok())
        .unwrap_or(1.0);

    let recording = args.iter().any(|arg| arg == "--recording");

    if recording {
        std::fs::create_dir_all("./recordings").unwrap();
    }

    let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER).unwrap();

    let (processor, _amp, writer) = Processor::new(&client, gain, recording);
    let process = processor.into_process_handler();

    let _active_client = client
        .activate_async(Notifications, jack::ClosureProcessHandler::new(process))
        .unwrap();

    println!(
        "🔥 Rustortion: Metal mode active (gain {:.2}){}!",
        gain,
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
