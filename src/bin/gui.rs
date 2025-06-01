use anyhow::{Context, Result};
use log::info;
use rustortion::gui::amp::start;
use rustortion::io::manager::ProcessorManager;

pub fn main() -> Result<()> {
    dotenv::dotenv().ok();
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

    // Check required environment variables (same as CLI)
    let required_vars = ["RUST_LOG", "PIPEWIRE_LATENCY", "JACK_PROMISCUOUS_SERVER"];
    for &key in &required_vars {
        match std::env::var(key) {
            Ok(val) => info!("{} = {}", key, val),
            Err(_) => anyhow::bail!("environment variable '{}' must be set.", key),
        }
    }

    // Create ProcessorManager with proper error handling
    let processor_manager = ProcessorManager::new().context("failed to create ProcessorManager")?;

    info!("ProcessorManager created successfully, starting GUI...");

    // Start the GUI with the processor manager
    start(processor_manager).map_err(|e| anyhow::anyhow!("GUI error: {}", e))?;

    Ok(())
}
