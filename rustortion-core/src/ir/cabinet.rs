use log::debug;
use serde::{Deserialize, Serialize};

use crate::ir::convolver::Convolver;

/// Configuration for convolver type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConvolverType {
    #[default]
    Fir,
    TwoStage,
}

/// Default maximum IR length in milliseconds for truncation
pub const DEFAULT_MAX_IR_MS: usize = 50;

pub struct IrCabinet {
    convolver: Convolver,

    bypassed: bool,
    output_gain: f32,
}

impl IrCabinet {
    pub fn new(convolver_type: ConvolverType, max_ir_samples: usize) -> Self {
        let convolver = match convolver_type {
            ConvolverType::Fir => Convolver::new_fir(max_ir_samples),
            ConvolverType::TwoStage => Convolver::new_two_stage(),
        };

        debug!("IrCabinet created: {convolver_type:?} convolver, max {max_ir_samples} samples");

        Self {
            convolver,
            bypassed: false,
            output_gain: 0.1,
        }
    }

    pub const fn swap_convolver(&mut self, convolver: Convolver) -> Convolver {
        std::mem::replace(&mut self.convolver, convolver)
    }

    pub fn clear_convolver(&mut self) {
        self.convolver.reset();
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        if self.bypassed {
            return;
        }

        self.convolver.process_block(samples);

        // Apply gain
        for sample in samples.iter_mut() {
            *sample *= self.output_gain;
        }
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if self.bypassed {
            return input;
        }

        let conv_out = self.convolver.process_sample(input);

        conv_out * self.output_gain
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
        if bypass {
            self.convolver.reset();
        }
    }

    pub const fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub const fn set_gain(&mut self, gain: f32) {
        self.output_gain = gain.clamp(0.0, 2.0);
    }

    pub const fn gain(&self) -> f32 {
        self.output_gain
    }
}
