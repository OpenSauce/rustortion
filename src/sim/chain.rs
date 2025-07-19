use crate::sim::stages::Stage;

// AmplifierChain implementation that holds and processes all stages
#[derive(Default)]
pub struct AmplifierChain {
    stages: Vec<Box<dyn Stage + Send>>,
    active_channel: usize,
    channel_mapping: Vec<(Vec<usize>, Vec<usize>, Vec<usize>)>, // Maps channel to (pre, channel-specific, post) stage indices
}

impl AmplifierChain {
    pub fn new() -> Self {
        Self {
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
