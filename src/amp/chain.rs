use crate::amp::stages::Stage;

// AmplifierChain holds a sequence of processing stages.
#[derive(Default)]
pub struct AmplifierChain {
    stages: Vec<Box<dyn Stage>>,
}

impl AmplifierChain {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn add_stage(&mut self, stage: Box<dyn Stage>) {
        self.stages.push(stage);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let mut signal = input;

        for stage in &mut self.stages {
            signal = stage.process(signal);
        }

        signal
    }

    // process_block processes a block of samples through the entire chain.
    pub fn process_block(&mut self, input: &mut [f32]) {
        for stage in &mut self.stages {
            stage.process_block(input);
        }
    }
}
