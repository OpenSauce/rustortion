use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

mod io;
mod sim;

use clap::Parser;
use io::manager::ProcessorManager;
use sim::chain::create_mesa_boogie_dual_rectifier;

#[derive(Parser, Debug)]
#[command(name = "rustortion")]
#[command(author = "OpenSauce")]
#[command(version = "0.1")]
#[command(about = "An amp sim with optional WAV recording.")]
struct Args {
    #[arg(long)]
    recording: bool,
}

fn main() -> Result<(), String> {
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "128/48000");
        env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
    }

    let args = Args::parse();
    let recording = args.recording;

    println!(
        "🔥 Rustortion: {}",
        if recording { "🛑 Recording!" } else { "" }
    );

    let mut processor_manager = ProcessorManager::new()?;

    let chain = create_mesa_boogie_dual_rectifier(processor_manager.sample_rate());

    if recording {
        processor_manager.enable_recording("./recordings")?;
    }

    processor_manager.start()?;

    processor_manager.set_amp_chain(chain);

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

    processor_manager.stop()?;

    Ok(())
}
