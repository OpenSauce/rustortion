pub mod fft;
pub mod fir;

pub use fft::TwoStageConvolver;
pub use fir::FirConvolver;

use anyhow::Result;

/// Convolver implementation selector
/// Ignore Clippy warning here, we are avoiding any dynamic dispatches using a box by using an enum, at the cost of some memory.
#[allow(clippy::large_enum_variant)]
pub enum Convolver {
    Fir(FirConvolver),
    TwoStage(TwoStageConvolver),
}

impl Convolver {
    pub fn new_fir(max_ir_length: usize) -> Self {
        Self::Fir(FirConvolver::new(max_ir_length))
    }

    pub fn new_two_stage() -> Self {
        Self::TwoStage(TwoStageConvolver::new())
    }

    pub fn set_ir(&mut self, ir: &[f32]) -> Result<()> {
        match self {
            Self::Fir(c) => c.set_ir(ir),
            Self::TwoStage(c) => c.set_ir(ir),
        }
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        match self {
            Self::Fir(c) => c.process_sample(input),
            Self::TwoStage(c) => c.process_sample(input),
        }
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        match self {
            Self::Fir(c) => c.process_block(samples),
            Self::TwoStage(c) => c.process_block(samples),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Fir(c) => c.reset(),
            Self::TwoStage(c) => c.reset(),
        }
    }
}
