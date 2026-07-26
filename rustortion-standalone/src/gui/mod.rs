pub mod app;
pub mod components;
pub mod handlers;

pub use app::AmplifierApp;
pub use rustortion_ui::messages::Message;

use std::cell::Cell;

use anyhow::{Result, anyhow};

use crate::settings::Settings;
use rustortion_ui::font::{EMBEDDED_FONT, EMBEDDED_FONT_BYTES};

pub fn start(settings: Settings) -> Result<()> {
    // Build the state up front so expected failures (no JACK server, unreadable
    // preset directory) surface through the caller's error path instead of
    // panicking inside iced's boot closure.
    let app = Cell::new(Some(AmplifierApp::new(settings)?));

    iced::application(
        // iced takes the boot closure by `Fn`, so the pre-built state is handed
        // over once. A program is only ever booted once, so the fallback is
        // unreachable.
        move || app.take().expect("iced booted the application twice"),
        AmplifierApp::update,
        AmplifierApp::view,
    )
    .subscription(AmplifierApp::subscription)
    .window(iced::window::Settings {
        maximized: true,
        min_size: Some(iced::Size::new(800.0, 600.0)),
        ..iced::window::Settings::default()
    })
    .font(EMBEDDED_FONT_BYTES)
    .default_font(EMBEDDED_FONT)
    .theme(AmplifierApp::theme)
    .title("Rustortion")
    .run()
    .map_err(|e| anyhow!("GUI error: {e}"))
}
