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

    /// Mirrors `convolver.has_ir()`. When false the cabinet short-circuits to
    /// pass-through, so a cleared cabinet behaves exactly like a bypassed one
    /// regardless of which convolver implementation is installed.
    has_ir: bool,

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
            has_ir: false,
            bypassed: false,
            output_gain: 0.1,
        }
    }

    /// RT-safe convolver swap: exchanges the cabinet's convolver with `other`
    /// in place. Neither side allocates or deallocates — the caller is left
    /// holding the previous convolver (e.g. to retire it off the RT thread).
    pub fn swap_convolver(&mut self, other: &mut Box<Convolver>) {
        std::mem::swap(&mut self.convolver, other);
        self.has_ir = self.convolver.has_ir();
    }

    /// Install a convolver by value, reusing the existing heap allocation.
    /// Intended for setup and tests, not the RT thread.
    pub fn set_convolver(&mut self, convolver: Convolver) {
        *self.convolver = convolver;
        self.has_ir = self.convolver.has_ir();
    }

    /// Removes the loaded IR. After this the cabinet is inert: audio passes
    /// through untouched, with no residual coefficients and no residual tail.
    ///
    /// RT-safe — neither convolver allocates or frees in `clear_ir`.
    pub fn clear_convolver(&mut self) {
        self.convolver.clear_ir();
        self.has_ir = false;
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        if self.bypassed || !self.has_ir {
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
        if self.bypassed || !self.has_ir {
            return input;
        }

        let conv_out = self.convolver.process_sample(input);

        conv_out * self.output_gain
    }

    /// Zero the convolver's delay line, dropping the cab's ringing tail while
    /// keeping the loaded IR (unlike [`Self::clear_convolver`], which discards
    /// the coefficients too).
    ///
    /// RT-safe: `Convolver::reset` allocates and frees nothing on either
    /// implementation.
    pub fn reset(&mut self) {
        self.convolver.reset();
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

    /// Whether an IR is currently loaded (i.e. the cabinet is doing anything).
    pub const fn has_ir(&self) -> bool {
        self.has_ir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decaying IR long enough to force the two-stage convolver to build a
    /// tail (> HEAD_LEN = 256), so both stages are exercised.
    fn test_ir() -> Vec<f32> {
        (0..1024)
            .map(|i| (-(i as f32) / 128.0).exp() * 0.5)
            .collect()
    }

    fn loaded_cabinet(convolver_type: ConvolverType) -> IrCabinet {
        let mut cab = IrCabinet::new(convolver_type, 4096);
        let mut convolver = match convolver_type {
            ConvolverType::Fir => Convolver::new_fir(4096),
            ConvolverType::TwoStage => Convolver::new_two_stage(),
        };
        convolver.set_ir(&test_ir()).unwrap();
        cab.set_convolver(convolver);
        cab.set_gain(1.0);
        cab
    }

    /// Assert two blocks are identical, reporting only the first mismatch —
    /// `assert_eq!` on 512-sample vectors dumps an unreadable wall of floats.
    fn assert_blocks_eq(actual: &[f32], expected: &[f32], context: &str) {
        assert_eq!(actual.len(), expected.len(), "{context}: length mismatch");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (a - e).abs() <= f32::EPSILON,
                "{context}: sample {i} differs: got {a}, expected {e}"
            );
        }
    }

    /// BUG-8 regression: `clear_convolver` used to call `Convolver::reset`,
    /// which zeroes the delay line but leaves the IR coefficients loaded — so
    /// a preset with `ir_name: None` kept convolving with the previous cab.
    #[test]
    fn clear_convolver_removes_the_ir() {
        for convolver_type in [ConvolverType::Fir, ConvolverType::TwoStage] {
            let mut cab = loaded_cabinet(convolver_type);

            // Prove the IR is actually doing something first.
            let mut warm = vec![0.0_f32; 512];
            warm[0] = 1.0;
            cab.process_block(&mut warm);
            assert!(
                (warm[1] - 0.0).abs() > 1e-6,
                "{convolver_type:?}: IR should be audible before clearing"
            );

            cab.clear_convolver();
            assert!(!cab.has_ir(), "{convolver_type:?}: has_ir after clear");

            // A block of known input must come out bit-identical: no residual
            // coefficients (would filter it) and no residual tail (would add
            // to it).
            let input: Vec<f32> = (0..512)
                .map(|i| ((i % 7) as f32).mul_add(0.1, -0.3))
                .collect();
            let mut block = input.clone();
            cab.process_block(&mut block);

            assert_blocks_eq(
                &block,
                &input,
                &format!("{convolver_type:?}: cleared cabinet must pass audio through unchanged"),
            );
        }
    }

    /// The cleared cabinet must match the bypassed cabinet sample-for-sample —
    /// that is the "no cab" reference path.
    #[test]
    fn cleared_cabinet_matches_bypass() {
        for convolver_type in [ConvolverType::Fir, ConvolverType::TwoStage] {
            let mut cleared = loaded_cabinet(convolver_type);
            cleared.clear_convolver();

            let mut bypassed = loaded_cabinet(convolver_type);
            bypassed.set_bypass(true);

            let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin()).collect();
            let mut a = input.clone();
            let mut b = input;
            cleared.process_block(&mut a);
            bypassed.process_block(&mut b);

            assert_blocks_eq(
                &a,
                &b,
                &format!("{convolver_type:?}: cleared should equal bypassed"),
            );
        }
    }

    /// Clearing must not wedge the cabinet: loading a new IR afterwards works.
    #[test]
    fn ir_can_be_reloaded_after_clear() {
        for convolver_type in [ConvolverType::Fir, ConvolverType::TwoStage] {
            let mut cab = loaded_cabinet(convolver_type);
            cab.clear_convolver();

            let mut convolver = match convolver_type {
                ConvolverType::Fir => Convolver::new_fir(4096),
                ConvolverType::TwoStage => Convolver::new_two_stage(),
            };
            convolver.set_ir(&test_ir()).unwrap();
            let mut boxed = Box::new(convolver);
            cab.swap_convolver(&mut boxed);

            assert!(cab.has_ir(), "{convolver_type:?}: has_ir after reload");

            let mut impulse = vec![0.0_f32; 512];
            impulse[0] = 1.0;
            cab.process_block(&mut impulse);
            assert!(
                (impulse[1] - 0.0).abs() > 1e-6,
                "{convolver_type:?}: reloaded IR should be audible"
            );
        }
    }
}
