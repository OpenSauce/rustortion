use std::{
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
use log::info;
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
    dotenv::dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    let recording = args.recording;

    info!(
        "Rustortion: {}",
        if recording { "🛑 Recording!" } else { "" }
    );

    let mut processor_manager = ProcessorManager::new()?;

    let chain = create_mesa_boogie_dual_rectifier(processor_manager.sample_rate());

    if recording {
        processor_manager.enable_recording("./recordings")?;
    }

    processor_manager.set_amp_chain(chain);

    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    ctrlc::set_handler(move || {
        info!("Ctrl+C received, shutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}
