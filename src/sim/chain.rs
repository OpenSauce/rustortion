use crate::sim::stages::clipper::ClipperType;
use crate::sim::stages::compressor::CompressorStage;
use crate::sim::stages::filter::FilterType;
use crate::sim::stages::poweramp::{PowerAmpStage, PowerAmpType};
use crate::sim::stages::tonestack::{ToneStackModel, ToneStackStage};
use crate::sim::stages::{Stage, filter::FilterStage, preamp::PreampStage};

// This example shows how to configure stages for a Mesa Boogie Dual Rectifier
pub fn create_mesa_boogie_dual_rectifier(sample_rate: f32) -> AmplifierChain {
    let mut chain = AmplifierChain::new("Mesa Boogie Dual Rectifier");

    // Input buffer/preamp filter - common to all channels
    chain.add_stage(Box::new(FilterStage::new(
        "Input Filter",
        FilterType::Highpass,
        80.0, // 80Hz highpass to cut rumble
        0.0,  // No resonance
        sample_rate,
    )));

    // CLEAN CHANNEL STAGES
    // Clean channel compressor - moderate compression
    chain.add_stage(Box::new(CompressorStage::new(
        "Clean Channel Compressor",
        1.0,   // 1ms attack - fast to catch transients
        150.0, // 150ms release - natural decay
        -20.0, // -20dB threshold - catches peaks
        3.0,   // 3:1 ratio - moderate compression
        2.0,   // +2dB makeup gain
        sample_rate,
    )));

    // Clean channel preamp
    chain.add_stage(Box::new(PreampStage::new(
        "Clean Preamp",
        2.5,               // Low gain
        0.0,               // No bias
        ClipperType::Soft, // Soft clipping for clean tones
    )));

    // Clean channel coupling capacitor (highpass filter)
    chain.add_stage(Box::new(FilterStage::new(
        "Clean Cap",
        FilterType::Highpass,
        120.0, // 120Hz coupling cap simulation
        0.0,   // No resonance
        sample_rate,
    )));

    // RHYTHM CHANNEL STAGES
    // Rhythm channel compressor
    chain.add_stage(Box::new(CompressorStage::new(
        "Rhythm Channel Compressor",
        0.5,   // 0.5ms attack - very fast
        100.0, // 100ms release
        -24.0, // -24dB threshold - more aggressive
        4.0,   // 4:1 ratio - tighter compression
        4.0,   // +4dB makeup gain
        sample_rate,
    )));

    // Rhythm preamp stage
    chain.add_stage(Box::new(PreampStage::new(
        "Rhythm Drive",
        5.0,                 // Medium gain
        0.1,                 // Slight positive bias
        ClipperType::Medium, // Medium clipping for rhythm
    )));

    // Rhythm coupling capacitor
    chain.add_stage(Box::new(FilterStage::new(
        "Rhythm Cap",
        FilterType::Highpass,
        150.0, // 150Hz to tighten bass
        0.0,   // No resonance
        sample_rate,
    )));

    // LEAD CHANNEL STAGES
    // Lead channel compressor
    chain.add_stage(Box::new(CompressorStage::new(
        "Lead Channel Compressor",
        0.5,   // 0.5ms attack - very fast
        100.0, // 100ms release
        -24.0, // -24dB threshold - more aggressive
        4.0,   // 4:1 ratio - tighter compression
        4.0,   // +4dB makeup gain
        sample_rate,
    )));

    // Lead preamp stage
    chain.add_stage(Box::new(PreampStage::new(
        "Lead Drive",
        8.0,                     // High gain
        0.2,                     // More positive bias for asymmetric clipping
        ClipperType::Asymmetric, // Asymmetric clipping for harmonically rich lead
    )));

    // Lead coupling capacitor
    chain.add_stage(Box::new(FilterStage::new(
        "Lead Cap",
        FilterType::Highpass,
        100.0, // 180Hz - tighter bass for lead
        0.0,   // No resonance
        sample_rate,
    )));

    // COMMON POST-PREAMP STAGES (shared by all channels)
    // Second gain stage (common to all channels)
    chain.add_stage(Box::new(PreampStage::new(
        "Secondary Gain",
        4.0,                 // Medium gain boost
        0.0,                 // No bias
        ClipperType::ClassA, // Class A tube behavior
    )));

    // Second stage coupling capacitor
    chain.add_stage(Box::new(FilterStage::new(
        "Secondary Cap",
        FilterType::Highpass,
        100.0, // 100Hz
        0.0,   // No resonance
        sample_rate,
    )));

    // Post-distortion compressor to tame peaks
    chain.add_stage(Box::new(CompressorStage::new(
        "Post Distortion Limiter",
        0.1,   // 0.1ms attack - extremely fast to catch peaks
        50.0,  // 50ms release - quick recovery
        -10.0, // -10dB threshold - only catches the highest peaks
        8.0,   // 8:1 ratio - more limiting than compression
        0.0,   // No makeup gain (we don't want to boost here)
        sample_rate,
    )));

    // Tone stack
    chain.add_stage(Box::new(ToneStackStage::new(
        "Mesa Tone Stack",
        ToneStackModel::Modern,
        0.5, // Bass
        0.4, // Mid (slightly scooped - Mesa characteristic)
        0.6, // Treble
        0.7, // Presence
        sample_rate,
    )));

    // Final filter before power amp
    chain.add_stage(Box::new(FilterStage::new(
        "Pre-Power Filter",
        FilterType::Lowpass,
        6500.0, // 6.5kHz lowpass to smooth harshness
        0.0,    // No resonance
        sample_rate,
    )));

    // Power amp simulation
    chain.add_stage(Box::new(PowerAmpStage::new(
        "Tube Power Amp",
        0.8, // Power amp drive
        PowerAmpType::ClassAB,
        0.6, // Sag (voltage sag under load)
        sample_rate,
    )));

    // Power amp compression (tube compression simulation)
    chain.add_stage(Box::new(CompressorStage::new(
        "Power Tube Compression",
        5.0,   // 5ms attack - tube compression is not instant
        250.0, // 250ms release - slow, tube-like recovery
        -16.0, // -16dB threshold
        6.0,   // 6:1 ratio - significant compression
        6.0,   // +6dB makeup gain - restore level
        sample_rate,
    )));

    // Output transformer/cabinet simulation (lowpass filter)
    chain.add_stage(Box::new(FilterStage::new(
        "Cabinet Simulation",
        FilterType::Lowpass,
        4200.0, // 4.2kHz - typical cab rolloff
        0.2,    // Slight resonance for speaker cone simulation
        sample_rate,
    )));

    // Define the three channels with their specific stages
    // Updated channel definitions with non-overlapping ranges:
    // Channel 0: Clean - Input filter (0) -> Clean stages (1,2,3) -> Common post stages (10+)
    chain.define_channel(
        0,
        vec![0],
        vec![1, 2, 3],
        vec![10, 11, 12, 13, 14, 15, 16, 17],
    );

    // Channel 1: Rhythm - Input filter (0) -> Rhythm stages (4,5,6) -> Common post stages (10+)
    chain.define_channel(
        1,
        vec![0],
        vec![4, 5, 6],
        vec![10, 11, 12, 13, 14, 15, 16, 17],
    );

    // Channel 2: Lead - Input filter (0) -> Lead stages (7,8,9) -> Common post stages (10+)
    chain.define_channel(
        2,
        vec![0],
        vec![7, 8, 9],
        vec![10, 11, 12, 13, 14, 15, 16, 17],
    );

    // Set default channel to clean
    chain.set_channel(0);

    chain
}

// AmplifierChain implementation that holds and processes all stages
pub struct AmplifierChain {
    #[allow(dead_code)]
    name: String,
    stages: Vec<Box<dyn Stage + Send>>,
    active_channel: usize, // 0, 1, or 2 for clean, rhythm, lead
    channel_mapping: Vec<(Vec<usize>, Vec<usize>, Vec<usize>)>, // Maps channel to (pre, channel-specific, post) stage indices
}

impl AmplifierChain {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stages: Vec::new(),
            active_channel: 0, // Default to clean channel
            channel_mapping: Vec::new(),
        }
    }

    pub fn add_stage(&mut self, stage: Box<dyn Stage>) {
        self.stages.push(stage);
    }

    pub fn set_channel(&mut self, channel: usize) {
        if channel < self.channel_mapping.len() {
            self.active_channel = channel;
        }
    }

    // Updated to use explicit stage indices for each channel
    pub fn define_channel(
        &mut self,
        channel: usize,
        pre_stages: Vec<usize>,
        channel_stages: Vec<usize>,
        post_stages: Vec<usize>,
    ) {
        if channel >= self.channel_mapping.len() {
            self.channel_mapping
                .resize(channel + 1, (Vec::new(), Vec::new(), Vec::new()));
        }
        self.channel_mapping[channel] = (pre_stages, channel_stages, post_stages);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let mut signal = input;

        // Process through active channel stages
        if !self.channel_mapping.is_empty() {
            let (pre_stages, channel_stages, post_stages) =
                &self.channel_mapping[self.active_channel];

            // Process pre-channel stages (common to all channels)
            for &stage_idx in pre_stages {
                if stage_idx < self.stages.len() {
                    signal = self.stages[stage_idx].process(signal);
                }
            }

            // Process active channel-specific stages
            for &stage_idx in channel_stages {
                if stage_idx < self.stages.len() {
                    signal = self.stages[stage_idx].process(signal);
                }
            }

            // Process post-channel stages (common to all channels)
            for &stage_idx in post_stages {
                if stage_idx < self.stages.len() {
                    signal = self.stages[stage_idx].process(signal);
                }
            }
        } else {
            // If no channel mapping defined, just process through all stages
            for stage in &mut self.stages {
                signal = stage.process(signal);
            }
        }

        signal
    }
}
