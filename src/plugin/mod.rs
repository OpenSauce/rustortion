use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::gui::config::StageConfig;
use crate::ir::cabinet::IrCabinet;
use crate::sim::chain::AmplifierChain;

mod params;

#[cfg(feature = "vst-gui")]
mod editor;

#[cfg(feature = "vst-gui-threaded")]
mod simple_window;

use params::RustortionParams;

pub struct Rustortion {
    params: Arc<RustortionParams>,
    chain: AmplifierChain,
    ir_cabinet: Option<IrCabinet>,
    sample_rate: f32,
}

impl Default for Rustortion {
    fn default() -> Self {
        Self {
            params: Arc::new(RustortionParams::default()),
            chain: AmplifierChain::new(),
            ir_cabinet: None,
            sample_rate: 48000.0,
        }
    }
}

impl Plugin for Rustortion {
    const NAME: &'static str = "Rustortion";
    const VENDOR: &'static str = "OpenSauce";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    // Embedded NIH-plug Iced GUI (older Iced version)
    #[cfg(all(feature = "vst-gui", not(feature = "vst-gui-threaded")))]
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    // Simple test window (separate OS window)
    #[cfg(feature = "vst-gui-threaded")]
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        eprintln!("RUSTORTION: editor() called - creating SimpleWindowEditor");
        println!("RUSTORTION: editor() called - creating SimpleWindowEditor");
        Some(Box::new(simple_window::SimpleWindowEditor::new(
            self.params.clone(),
        )))
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        // Initialize IR cabinet
        if let Ok(mut cabinet) = IrCabinet::new(
            std::path::Path::new("./impulse_responses"),
            buffer_config.sample_rate as u32,
        ) {
            // Try to load first available IR
            let ir_names = cabinet.get_loader().get_available_irs();
            if let Some(first_ir) = ir_names.first() {
                let _ = cabinet.select_ir(first_ir);
            }
            self.ir_cabinet = Some(cabinet);
        }

        // Initialize with default stages if none set
        self.rebuild_chain();

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check if stages have changed and rebuild chain if needed
        if self
            .params
            .stages_changed
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.rebuild_chain();
            self.params
                .stages_changed
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }

        // Process audio sample by sample
        for mut channel_samples in buffer.iter_samples() {
            // Get smoothed output gain for this sample
            let output_gain = self.params.output_gain.smoothed.next();

            // Sum all input channels to mono
            let mut input_sample = 0.0;
            let num_channels = channel_samples.len();

            // Read from input channels
            for sample in channel_samples.iter_mut() {
                input_sample += *sample;
            }

            // Average if multiple input channels
            if num_channels > 0 {
                input_sample /= num_channels as f32;
            }

            // Process through amplifier chain
            let mut processed = self.chain.process(input_sample);

            // Apply IR cabinet if available
            if let Some(ref mut cab) = self.ir_cabinet {
                let mut block = [processed];
                cab.process_block(&mut block);
                processed = block[0];
            }

            // Apply smoothed output gain
            processed *= output_gain;

            // Write processed sample to all output channels
            for sample in channel_samples.iter_mut() {
                *sample = processed;
            }
        }

        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {
        // Reset any state if needed
    }
}

impl Rustortion {
    fn rebuild_chain(&mut self) {
        self.chain = AmplifierChain::new();

        // Get stages from params
        if let Some(stages_json) = self.params.stages_json.lock().as_ref() {
            if let Ok(stages) = serde_json::from_str::<Vec<StageConfig>>(stages_json) {
                for stage_config in stages {
                    self.chain
                        .add_stage(stage_config.to_runtime(self.sample_rate));
                }
            }
        }
    }
}

impl ClapPlugin for Rustortion {
    const CLAP_ID: &'static str = "com.opensauce.rustortion";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Guitar amp simulator with cabinet IR");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Mono,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Rustortion {
    const VST3_CLASS_ID: [u8; 16] = *b"RustortionAmpSim";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(Rustortion);
nih_export_vst3!(Rustortion);
