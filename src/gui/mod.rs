pub mod app;
pub mod components;
pub mod config;
pub mod messages;
pub mod preset;
pub mod settings;

pub use app::AmplifierApp;
use iced::{Font, window};
pub use messages::Message;

use crate::{gui::settings::Settings, io::manager::ProcessorManager};

pub const DEFAULT_FONT: Font = Font::MONOSPACE;

pub fn start(processor_manager: ProcessorManager, settings: Settings) -> iced::Result {
    iced::application("Rustortion", AmplifierApp::update, AmplifierApp::view)
        .subscription(AmplifierApp::subscription)
        .window_size((800.0, 600.0))
        .theme(AmplifierApp::theme)
        .default_font(DEFAULT_FONT)
        .run_with(move || {
            (
                AmplifierApp::new(processor_manager, settings),
                window::get_latest().and_then(|id| window::maximize(id, true)),
            )
        })
}
