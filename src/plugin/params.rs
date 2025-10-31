use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[cfg(feature = "vst-gui")]
use nih_plug_iced::IcedState;

#[derive(Params)]
pub struct RustortionParams {
    /// Editor state for the GUI
    #[cfg(feature = "vst-gui")]
    #[persist = "editor-state"]
    pub editor_state: Arc<IcedState>,

    /// Output gain control
    #[id = "output_gain"]
    pub output_gain: FloatParam,

    /// JSON representation of the current stage chain
    /// This is how we serialize the entire effect chain
    #[persist = "stages"]
    pub stages_json: Arc<Mutex<Option<String>>>,

    /// Flag to indicate stages have changed and need rebuilding
    #[persist = "stages_changed"]
    pub stages_changed: Arc<AtomicBool>,
}

impl Default for RustortionParams {
    fn default() -> Self {
        Self {
            #[cfg(feature = "vst-gui")]
            editor_state: {
                #[cfg(feature = "vst-gui")]
                {
                    crate::plugin::editor::default_state()
                }
            },

            output_gain: FloatParam::new(
                "Output Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            stages_json: Arc::new(Mutex::new(None)),
            stages_changed: Arc::new(AtomicBool::new(false)),
        }
    }
}
