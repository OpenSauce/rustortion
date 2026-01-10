pub mod fft;
pub mod fir;

pub use fft::TwoStageConvolver;
pub use fir::FirConvolver;

use anyhow::Result;

/// Convolver implementation selector
/// Ignore Clippy warning here so we use enum dispatch for performance
#[allow(clippy::large_enum_variant)]
pub enum Convolver {
    Fir(FirConvolver),
    TwoStage(TwoStageConvolver),
}

impl Convolver {
    pub fn new_fir(max_ir_length: usize) -> Self {
        Convolver::Fir(FirConvolver::new(max_ir_length))
    }

    pub fn new_two_stage() -> Self {
        Convolver::TwoStage(TwoStageConvolver::new())
    }

    pub fn set_ir(&mut self, ir: &[f32]) -> Result<()> {
        match self {
            Convolver::Fir(c) => c.set_ir(ir),
            Convolver::TwoStage(c) => c.set_ir(ir),
        }
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        match self {
            Convolver::Fir(c) => c.process_sample(input),
            Convolver::TwoStage(c) => c.process_sample(input),
        }
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        match self {
            Convolver::Fir(c) => c.process_block(samples),
            Convolver::TwoStage(c) => c.process_block(samples),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Convolver::Fir(c) => c.reset(),
            Convolver::TwoStage(c) => c.reset(),
        }
    }
}
