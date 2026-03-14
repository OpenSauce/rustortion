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

    /// Forward a parameter change to a live stage.
    pub fn set_parameter(
        &mut self,
        idx: usize,
        name: &str,
        value: f32,
    ) -> Option<Result<(), &'static str>> {
        self.stages
            .get_mut(idx)
            .map(|stage| stage.set_parameter(name, value))
    }

    /// Read a parameter from a live stage.
    pub fn get_parameter(&self, idx: usize, name: &str) -> Option<Result<f32, &'static str>> {
        self.stages.get(idx).map(|stage| stage.get_parameter(name))
    }

    /// Insert a stage at the given index.
    pub fn insert_stage(&mut self, idx: usize, stage: Box<dyn Stage>) {
        let idx = idx.min(self.stages.len());
        self.stages.insert(idx, stage);
    }

    /// Remove and return the stage at the given index.
    pub fn remove_stage(&mut self, idx: usize) -> Option<Box<dyn Stage>> {
        if idx < self.stages.len() {
            Some(self.stages.remove(idx))
        } else {
            None
        }
    }

    /// Swap two stages by index.
    pub fn swap_stages(&mut self, a: usize, b: usize) {
        if a < self.stages.len() && b < self.stages.len() {
            self.stages.swap(a, b);
        }
    }

    /// Replace a stage at the given index, returning the old one.
    pub fn replace_stage(
        &mut self,
        idx: usize,
        new_stage: Box<dyn Stage>,
    ) -> Option<Box<dyn Stage>> {
        if idx < self.stages.len() {
            let old = std::mem::replace(&mut self.stages[idx], new_stage);
            Some(old)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amp::stages::level::LevelStage;

    fn make_level(gain: f32) -> Box<dyn Stage> {
        Box::new(LevelStage::new(gain))
    }

    #[test]
    fn set_parameter_updates_live_stage() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(1.0));
        assert!(chain.set_parameter(0, "gain", 0.5).is_some());
        let out = chain.process(1.0);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn set_parameter_out_of_bounds_returns_none() {
        let mut chain = AmplifierChain::new();
        assert!(chain.set_parameter(0, "gain", 0.5).is_none());
    }

    #[test]
    fn insert_stage_at_position() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(1.0));
        chain.add_stage(make_level(1.0));
        chain.insert_stage(1, make_level(0.5));
        let out = chain.process(1.0);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn remove_stage_returns_removed() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.add_stage(make_level(1.0));
        let removed = chain.remove_stage(0);
        assert!(removed.is_some());
        let out = chain.process(1.0);
        assert!((out - 1.0).abs() < 1e-6);
    }

    #[test]
    fn remove_stage_out_of_bounds() {
        let mut chain = AmplifierChain::new();
        assert!(chain.remove_stage(5).is_none());
    }

    #[test]
    fn swap_stages_changes_order() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.add_stage(make_level(2.0));
        chain.swap_stages(0, 1);
        assert!((chain.get_parameter(0, "gain").unwrap().unwrap() - 2.0).abs() < 1e-6);
        assert!((chain.get_parameter(1, "gain").unwrap().unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn replace_stage_preserves_others() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.add_stage(make_level(1.0));
        let old = chain.replace_stage(0, make_level(0.25));
        assert!(old.is_some());
        let out = chain.process(1.0);
        assert!((out - 0.25).abs() < 1e-6);
    }
}
