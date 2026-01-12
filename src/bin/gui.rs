use anyhow::Result;
use log::info;
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
    info!("v{}", env!("CARGO_PKG_VERSION"));
    info!("{}", settings);

    start(settings).map_err(|e| anyhow::anyhow!("GUI error: {}", e))?;

    Ok(())
}
