pub mod app;
pub mod components;
pub mod handlers;

pub use app::AmplifierApp;
pub use rustortion_ui::messages::Message;

use crate::settings::Settings;
use rustortion_ui::font::{EMBEDDED_FONT, EMBEDDED_FONT_BYTES};

pub fn start(settings: Settings) -> iced::Result {
    iced::application(
        move || AmplifierApp::boot(settings.clone()),
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
}
