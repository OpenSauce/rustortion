pub mod app;
pub mod components;
pub mod config;
pub mod handlers;
pub mod messages;
pub mod settings;

pub use app::AmplifierApp;
use iced::{Font, window};
pub use messages::Message;

use crate::{audio::manager::Manager, gui::settings::Settings};

pub const DEFAULT_FONT: Font = Font::MONOSPACE;

pub fn start(audio_manager: Manager, settings: Settings) -> iced::Result {
    iced::application("Rustortion", AmplifierApp::update, AmplifierApp::view)
        .subscription(AmplifierApp::subscription)
        .window(iced::window::Settings {
            min_size: Some(iced::Size::new(800.0, 600.0)),
            ..iced::window::Settings::default()
        })
        .theme(AmplifierApp::theme)
        .default_font(DEFAULT_FONT)
        .run_with(move || {
            (
                AmplifierApp::new(audio_manager, settings),
                window::get_oldest().and_then(|id| window::maximize(id, true)),
            )
        })
}
