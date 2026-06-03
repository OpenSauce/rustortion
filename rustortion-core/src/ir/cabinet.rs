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
    /// Boxed so the convolver can be swapped in/out on the RT thread by
    /// exchanging pointers (`swap_convolver`) without moving the heavy
    /// convolver struct or allocating to type-erase it for `rt_drop`.
    convolver: Box<Convolver>,

    bypassed: bool,
    output_gain: f32,
}

impl IrCabinet {
    pub fn new(convolver_type: ConvolverType, max_ir_samples: usize) -> Self {
        let convolver = Box::new(match convolver_type {
            ConvolverType::Fir => Convolver::new_fir(max_ir_samples),
            ConvolverType::TwoStage => Convolver::new_two_stage(),
        });

        debug!("IrCabinet created: {convolver_type:?} convolver, max {max_ir_samples} samples");

        Self {
            convolver,
            bypassed: false,
            output_gain: 0.1,
        }
    }

    /// RT-safe convolver swap: exchanges the cabinet's convolver with `other`
    /// in place. Neither side allocates or deallocates — the caller is left
    /// holding the previous convolver (e.g. to retire it off the RT thread).
    pub const fn swap_convolver(&mut self, other: &mut Box<Convolver>) {
        std::mem::swap(&mut self.convolver, other);
    }

    /// Install a convolver by value, reusing the existing heap allocation.
    /// Intended for setup and tests, not the RT thread.
    pub fn set_convolver(&mut self, convolver: Convolver) {
        *self.convolver = convolver;
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
