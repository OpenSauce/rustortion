pub mod app;
pub mod components;
pub mod config;
pub mod messages;

pub use app::AmplifierApp;
pub use messages::Message;

use crate::io::manager::ProcessorManager;

pub fn start(processor_manager: ProcessorManager) -> iced::Result {
    iced::application("Rustortion", AmplifierApp::update, AmplifierApp::view)
        .window_size((800.0, 600.0))
        .theme(AmplifierApp::theme)
        .run_with(move || (AmplifierApp::new(processor_manager), iced::Task::none()))
}
