pub mod app;
pub mod components;
pub mod config;
pub mod handlers;
pub mod messages;

pub use app::AmplifierApp;
pub use messages::Message;

use crate::settings::Settings;

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
    .theme(AmplifierApp::theme)
    .title("Rustortion")
    .run()
}
