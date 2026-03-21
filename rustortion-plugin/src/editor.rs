use std::collections::HashMap;
use std::sync::Arc;

use nih_plug::prelude::{Editor, GuiContext};

use crate::SharedState;
use crate::backend::PluginBackend;
use crate::params::RustortionParams;

use rustortion_ui::app::{SharedApp, UpdateResult};
use rustortion_ui::backend::ParamBackend;
use rustortion_ui::components::ir_cabinet_control::IrCabinetControl;
use rustortion_ui::components::peak_meter::PeakMeterDisplay;
use rustortion_ui::components::pitch_shift_control::PitchShiftControl;
use rustortion_ui::handlers::hotkey::HotkeyHandler;
use rustortion_ui::handlers::preset::PresetHandler;
use rustortion_ui::hotkey::HotkeySettings;
use rustortion_ui::messages::Message;
use rustortion_ui::stages::StageType;
use rustortion_ui::tabs::Tab;

// ---------------------------------------------------------------------------
// Send wrapper for iced_baseview::WindowHandle
// ---------------------------------------------------------------------------

/// Wrapper around `iced_baseview::WindowHandle` to satisfy nih-plug's
/// `Box<dyn Any + Send>` requirement for `Editor::spawn`. The window handle
/// contains raw pointers (X11 window ID, etc.) that are not `Send` by default,
/// but in practice the handle is only held as a drop guard by the host and is
/// never moved across threads.
struct SendWindowHandle<M: 'static + Send>(
    #[allow(dead_code)] iced_baseview::window::WindowHandle<M>,
);

// SAFETY: The WindowHandle is only stored as a drop guard. The raw pointers it
// contains (X11 display, etc.) are not accessed from other threads.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<M: 'static + Send> Send for SendWindowHandle<M> {}

// ---------------------------------------------------------------------------
// nih-plug Editor implementation
// ---------------------------------------------------------------------------

pub struct PluginEditor {
    params: Arc<RustortionParams>,
    shared_state: Arc<SharedState>,
}

impl PluginEditor {
    pub const fn new(params: Arc<RustortionParams>, shared_state: Arc<SharedState>) -> Self {
        Self {
            params,
            shared_state,
        }
    }
}

impl Editor for PluginEditor {
    fn spawn(
        &self,
        parent: nih_plug::editor::ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        // Gather engine state for the backend
        let engine_handle = self
            .shared_state
            .engine_handle
            .lock()
            .ok()
            .and_then(|g| g.clone());
        let ir_loader = self
            .shared_state
            .ir_loader
            .lock()
            .ok()
            .and_then(|g| g.clone());
        let sample_rate = f32::from_bits(
            self.shared_state
                .sample_rate
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        let os_idx = self
            .shared_state
            .oversampling_idx
            .load(std::sync::atomic::Ordering::Relaxed);
        let oversampling_factor = 2_u32.pow(u32::from(os_idx));

        let flags = PluginAppFlags {
            params: self.params.clone(),
            context,
            engine_handle,
            ir_loader,
            sample_rate,
            oversampling_factor,
        };

        let settings = iced_baseview::Settings {
            window: iced_baseview::baseview::WindowOpenOptions {
                title: String::from("Rustortion"),
                size: iced_baseview::baseview::Size::new(900.0, 700.0),
                scale: iced_baseview::baseview::WindowScalePolicy::SystemScaleFactor,
            },
            graphics_settings: iced_baseview::graphics::Settings::default(),
            iced_baseview: iced_baseview::settings::IcedBaseviewSettings::default(),
            ..Default::default()
        };

        let handle = iced_baseview::open_parented::<PluginApp, _>(&parent, flags, settings);

        Box::new(SendWindowHandle(handle))
    }

    fn size(&self) -> (u32, u32) {
        (900, 700)
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        // We use SystemScaleFactor from baseview; accept but don't
        // manually resize.
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        // SharedApp reads parameter values on each view(); no explicit
        // notification plumbing needed.
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}

    fn param_values_changed(&self) {}
}

// ---------------------------------------------------------------------------
// iced_baseview Application
// ---------------------------------------------------------------------------

struct PluginAppFlags {
    params: Arc<RustortionParams>,
    context: Arc<dyn GuiContext>,
    engine_handle: Option<rustortion_core::audio::engine::EngineHandle>,
    ir_loader: Option<Arc<rustortion_core::ir::loader::IrLoader>>,
    sample_rate: f32,
    oversampling_factor: u32,
}

struct PluginApp {
    shared: SharedApp<PluginBackend>,
}

impl iced_baseview::Application for PluginApp {
    type Message = Message;
    type Theme = iced_baseview::Theme;
    type Executor = iced_baseview::executor::Default;
    type Flags = PluginAppFlags;

    fn new(flags: Self::Flags) -> (Self, iced_baseview::Task<Self::Message>) {
        let engine_handle = flags.engine_handle.expect(
            "PluginApp::new called without an engine handle; \
             the plugin must be initialized before the editor opens",
        );

        let backend = PluginBackend::new(
            engine_handle,
            flags.params,
            flags.context,
            flags.ir_loader,
            flags.sample_rate,
            flags.oversampling_factor,
        );

        let available_irs = backend.get_available_irs();

        let preset_dir = dirs::config_dir()
            .unwrap_or_default()
            .join("rustortion")
            .join("presets");
        let preset_handler = PresetHandler::new(&preset_dir)
            .unwrap_or_else(|_| PresetHandler::new("/dev/null").unwrap());

        let mut ir_cabinet = IrCabinetControl::default();
        ir_cabinet.set_available_irs(available_irs);

        let shared = SharedApp {
            backend,
            stages: Vec::new(),
            collapsed_stages: Vec::new(),
            dirty_params: HashMap::new(),
            active_tab: Tab::Amp,
            selected_stage_type: StageType::ALL.first().copied().unwrap_or(StageType::Preamp),
            ir_cabinet_control: ir_cabinet,
            pitch_shift_control: PitchShiftControl::new(0),
            preset_handler,
            peak_meter_display: PeakMeterDisplay::default(),
            hotkey_handler: HotkeyHandler::new(HotkeySettings::default()),
            input_filter_config: rustortion_core::preset::InputFilterConfig::default(),
            is_recording: false,
        };

        (Self { shared }, iced_baseview::Task::none())
    }

    fn update(&mut self, message: Self::Message) -> iced_baseview::Task<Self::Message> {
        match self.shared.update(message) {
            UpdateResult::Handled(task) => task,
            UpdateResult::Unhandled(_msg) => {
                // Standalone-only messages (Settings, Midi, Tuner, Recording)
                // are silently dropped in plugin mode.
                iced_baseview::Task::none()
            }
        }
    }

    fn view(
        &self,
    ) -> iced_baseview::Element<'_, Self::Message, Self::Theme, iced_baseview::Renderer> {
        self.shared.view()
    }

    fn theme(&self) -> Self::Theme {
        iced_baseview::Theme::TokyoNight
    }

    fn subscription(
        &self,
        _window_subs: &mut iced_baseview::WindowSubs<Self::Message>,
    ) -> iced_baseview::futures::Subscription<Self::Message> {
        self.shared.subscription()
    }
}
