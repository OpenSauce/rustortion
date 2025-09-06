use crate::sim::stages::Stage;

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
}
