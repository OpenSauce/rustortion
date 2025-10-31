use nih_plug::prelude::*;
use std::sync::Arc;
use std::thread;

use super::params::RustortionParams;

pub struct SimpleWindowEditor {
    params: Arc<RustortionParams>,
}

impl SimpleWindowEditor {
    pub fn new(params: Arc<RustortionParams>) -> Self {
        Self { params }
    }
}

impl Editor for SimpleWindowEditor {
    fn spawn(
        &self,
        _parent: ParentWindowHandle,
        _context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        eprintln!("RUSTORTION: spawn() called!");

        let params = self.params.clone();

        // Spawn thread that opens a window
        thread::spawn(move || {
            eprintln!("RUSTORTION: Thread started!");

            // Set env var to allow event loop on any thread
            unsafe {
                std::env::set_var("WINIT_UNIX_BACKEND", "x11");
            }

            // Try to open the GUI
            if let Err(e) = open_gui_window(params) {
                eprintln!("RUSTORTION: Error opening GUI: {}", e);
            }
        });

        Box::new(())
    }

    fn size(&self) -> (u32, u32) {
        (0, 0) // External window, no embedded size
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
    fn param_values_changed(&self) {}
}

fn open_gui_window(params: Arc<RustortionParams>) -> Result<(), iced::Error> {
    eprintln!("RUSTORTION: open_gui_window() starting");

    use iced::widget::{button, column, container, text};
    use iced::{Element, Font, Length, Task, Theme, window};

    #[derive(Debug, Clone)]
    enum Message {
        ButtonPressed,
    }

    struct SimpleApp {
        counter: i32,
    }

    impl SimpleApp {
        fn new() -> Self {
            eprintln!("RUSTORTION: SimpleApp created");
            Self { counter: 0 }
        }

        fn update(&mut self, message: Message) -> Task<Message> {
            match message {
                Message::ButtonPressed => {
                    self.counter += 1;
                    eprintln!("RUSTORTION: Button pressed! Counter: {}", self.counter);
                }
            }
            Task::none()
        }

        fn view(&self) -> Element<Message> {
            container(
                column![
                    text("Rustortion Plugin GUI").size(32),
                    text(format!("Counter: {}", self.counter)).size(24),
                    button(text("Click Me!")).on_press(Message::ButtonPressed),
                    text("This is a separate window!"),
                ]
                .spacing(20)
                .padding(20),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        }
    }

    eprintln!("RUSTORTION: Starting Iced application");

    // Just use regular Iced run - it should work on a thread with X11
    let result = iced::application("Rustortion Test", SimpleApp::update, SimpleApp::view)
        .window(window::Settings {
            size: iced::Size::new(400.0, 300.0),
            decorations: true,
            ..window::Settings::default()
        })
        .theme(|_| Theme::Dark)
        .run_with(|| {
            eprintln!("RUSTORTION: Iced run_with callback");
            (SimpleApp::new(), Task::none())
        });

    match &result {
        Ok(_) => eprintln!("RUSTORTION: Iced app completed"),
        Err(e) => eprintln!("RUSTORTION: Iced error: {}", e),
    }

    result
}
