use anyhow::{Context, Result};
use log::info;
use rustortion::audio::manager::Manager;
use rustortion::gui::start;
use rustortion::settings::Settings;

pub fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let settings = Settings::load().unwrap_or_else(|e| {
        info!("Could not load settings, using defaults: {}", e);
        Settings::default()
    });

    settings.apply_to_environment();

    env_logger::init();

    info!("Rustortion GUI v{}", env!("CARGO_PKG_VERSION"));
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

    let required_vars = ["RUST_LOG", "PIPEWIRE_LATENCY", "JACK_PROMISCUOUS_SERVER"];
    for &key in &required_vars {
        match std::env::var(key) {
            Ok(val) => info!("{key} = {val}"),
            Err(_) => anyhow::bail!("environment variable '{}' must be set.", key),
        }
    }

    info!("Audio Settings:");
    info!("  Input: {}", settings.audio.input_port);
    info!("  Output L: {}", settings.audio.output_left_port);
    info!("  Output R: {}", settings.audio.output_right_port);
    info!("  Buffer Size: {}", settings.audio.buffer_size);
    info!("  Sample Rate: {}", settings.audio.sample_rate);
    info!("  Auto-connect: {}", settings.audio.auto_connect);

    let audio_manager =
        Manager::new(settings.clone()).context("failed to create ProcessorManager")?;

    start(audio_manager, settings).map_err(|e| anyhow::anyhow!("GUI error: {}", e))?;

    Ok(())
}
