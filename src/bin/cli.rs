use anyhow::{Context, Result};
use clap::Parser;
use log::info;
use rustortion::io::manager::ProcessorManager;
use rustortion::sim::chain::create_mesa_boogie_dual_rectifier;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Parser, Debug)]
#[command(name = "rustortion")]
#[command(author = "OpenSauce")]
#[command(version = "0.1")]
#[command(about = "An amp sim with optional WAV recording.")]
struct Args {
    #[arg(long, help = "Enable WAV recording")]
    recording: bool,
    #[arg(
        long,
        env = "RECORDING_DIR",
        default_value = "./recordings",
        help = "Directory to save recordings"
    )]
    recording_dir: String,
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    let recording = args.recording;

    info!("Rustortion v{}", env!("CARGO_PKG_VERSION"));
    info!(
        r#"
__________                __                 __  .__               
\______   \__ __  _______/  |_  ____________/  |_|__| ____   ____  
 |       _/  |  \/  ___/\   __\/  _ \_  __ \   __\  |/  _ \ /    \ 
 |    |   \  |  /\___ \  |  | (  <_> )  | \/|  | |  (  <_> )   |  \
 |____|_  /____//____  > |__|  \____/|__|   |__| |__|\____/|___|  /
        \/           \/                                         \/ 
    "#
    );
    info!("Args: {:?}", args);

    let required_vars = ["RUST_LOG", "PIPEWIRE_LATENCY", "JACK_PROMISCUOUS_SERVER"];
    for &key in &required_vars {
        match std::env::var(key) {
            Ok(val) => info!("{} = {}", key, val),
            Err(_) => anyhow::bail!("environment variable '{}' must be set.", key),
        }
    }

    let mut processor_manager =
        ProcessorManager::new().context("failed to create ProcessorManager")?;

    let chain = create_mesa_boogie_dual_rectifier(processor_manager.sample_rate());

    if recording {
        processor_manager
            .enable_recording(&args.recording_dir)
            .with_context(|| format!("failed to enable recording in '{}'", args.recording_dir))?;
    }

    processor_manager.set_amp_chain(chain);

    let running = Arc::new(AtomicBool::new(true));
    let shutdown_flag = Arc::clone(&running);

    ctrlc::set_handler(move || {
        info!("Ctrl+C received, shutting down...");
        shutdown_flag.store(false, Ordering::SeqCst);
    })
    .expect("error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}
