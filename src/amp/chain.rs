use crate::amp::stages::Stage;

struct BypassableStage {
    inner: Box<dyn Stage>,
    bypassed: bool,
}

// AmplifierChain holds a sequence of processing stages.
#[derive(Default)]
pub struct AmplifierChain {
    stages: Vec<BypassableStage>,
}

impl AmplifierChain {
    #[must_use]
    pub const fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn add_stage(&mut self, stage: Box<dyn Stage>) {
        self.stages.push(BypassableStage { inner: stage, bypassed: false });
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let mut signal = input;

        for stage in &mut self.stages {
            if !stage.bypassed {
                signal = stage.inner.process(signal);
            }
        }

        signal
    }

    // process_block processes a block of samples through the entire chain.
    pub fn process_block(&mut self, input: &mut [f32]) {
        for stage in &mut self.stages {
            if !stage.bypassed {
                stage.inner.process_block(input);
            }
        }
    }

    /// Forward a parameter change to a live stage.
    pub fn set_parameter(
        &mut self,
        idx: usize,
        name: &str,
        value: f32,
    ) -> Option<Result<(), &'static str>> {
        self.stages.get_mut(idx).map(|s| s.inner.set_parameter(name, value))
    }

    /// Read a parameter from a live stage.
    pub fn get_parameter(&self, idx: usize, name: &str) -> Option<Result<f32, &'static str>> {
        self.stages.get(idx).map(|s| s.inner.get_parameter(name))
    }

    /// Insert a stage at the given index.
    pub fn insert_stage(&mut self, idx: usize, stage: Box<dyn Stage>) {
        let idx = idx.min(self.stages.len());
        self.stages.insert(idx, BypassableStage { inner: stage, bypassed: false });
    }

    /// Remove and return the stage at the given index.
    pub fn remove_stage(&mut self, idx: usize) -> Option<Box<dyn Stage>> {
        if idx < self.stages.len() {
            Some(self.stages.remove(idx).inner)
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
            let old = std::mem::replace(&mut self.stages[idx].inner, new_stage);
            Some(old)
        } else {
            None
        }
    }

    /// Set the bypass state of a stage.
    pub fn set_bypassed(&mut self, idx: usize, bypassed: bool) {
        if let Some(stage) = self.stages.get_mut(idx) {
            stage.bypassed = bypassed;
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

    #[test]
    fn bypassed_stage_passes_signal_through() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5)); // halves signal
        chain.add_stage(make_level(1.0));
        chain.set_bypassed(0, true);
        let out = chain.process(1.0);
        assert!((out - 1.0).abs() < 1e-6, "bypassed stage should pass through");
    }

    #[test]
    fn bypassed_stage_block_passes_through() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.set_bypassed(0, true);
        let mut buf = [1.0_f32; 4];
        chain.process_block(&mut buf);
        for s in &buf {
            assert!((*s - 1.0).abs() < 1e-6, "bypassed block should pass through");
        }
    }

    #[test]
    fn unbypassed_stage_processes_normally() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.set_bypassed(0, true);
        chain.set_bypassed(0, false);
        let out = chain.process(1.0);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn add_stage_initializes_bypass_false() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(1.0));
        // Should not be bypassed by default
        let out = chain.process(1.0);
        assert!((out - 1.0).abs() < 1e-6);
    }

    #[test]
    fn insert_stage_keeps_bypass_in_sync() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(1.0));
        chain.set_bypassed(0, true);
        chain.insert_stage(0, make_level(0.5)); // insert before bypassed
        // idx 0 = new (active, 0.5x), idx 1 = old (bypassed, 1.0x)
        let out = chain.process(1.0);
        assert!((out - 0.5).abs() < 1e-6, "inserted stage active, old stage bypassed");
    }

    #[test]
    fn remove_stage_keeps_bypass_in_sync() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(1.0));
        chain.add_stage(make_level(0.5));
        chain.set_bypassed(1, true);
        chain.remove_stage(0); // remove first; now the bypassed one is at idx 0
        let out = chain.process(1.0);
        assert!((out - 1.0).abs() < 1e-6, "remaining bypassed stage should pass through");
    }

    #[test]
    fn swap_stages_swaps_bypass_state() {
        let mut chain = AmplifierChain::new();
        chain.add_stage(make_level(0.5));
        chain.add_stage(make_level(2.0));
        chain.set_bypassed(0, true); // bypass the 0.5x stage
        chain.swap_stages(0, 1);
        // After swap: idx 0 = 2.0x (not bypassed), idx 1 = 0.5x (bypassed)
        let out = chain.process(1.0);
        assert!((out - 2.0).abs() < 1e-6, "swapped: active 2x, bypassed 0.5x");
    }
}
