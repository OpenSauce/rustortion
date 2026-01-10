use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ir::convolver::Convolver;
use crate::ir::loader::IrLoader;

/// Configuration for convolver type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ConvolverType {
    #[default]
    Fir,
    TwoStage,
}

/// Default maximum IR length in milliseconds for truncation
const DEFAULT_MAX_IR_MS: usize = 50;

pub struct IrCabinet {
    ir_loader: IrLoader,
    convolver: Convolver,
    convolver_type: ConvolverType,
    sample_rate: usize,
    max_ir_samples: usize,

    bypassed: bool,
    output_gain: f32,
}

impl IrCabinet {
    pub fn new(ir_directory: &Path, sample_rate: usize) -> Result<Self> {
        Self::with_config(
            ir_directory,
            sample_rate,
            ConvolverType::default(),
            DEFAULT_MAX_IR_MS,
        )
    }

    pub fn with_config(
        ir_directory: &Path,
        sample_rate: usize,
        convolver_type: ConvolverType,
        max_ir_ms: usize,
    ) -> Result<Self> {
        let ir_loader = IrLoader::new(ir_directory, sample_rate)?;
        let max_ir_samples = (sample_rate * max_ir_ms) / 1000;

        let convolver = match convolver_type {
            ConvolverType::Fir => Convolver::new_fir(max_ir_samples),
            ConvolverType::TwoStage => Convolver::new_two_stage(),
        };

        info!(
            "IrCabinet created: {:?} convolver, max {}ms ({} samples)",
            convolver_type, max_ir_ms, max_ir_samples
        );

        Ok(Self {
            ir_loader,
            convolver,
            convolver_type,
            sample_rate,
            max_ir_samples,
            bypassed: false,
            output_gain: 1.0,
        })
    }

    pub fn available_ir_names(&self) -> Vec<String> {
        self.ir_loader.available_ir_names()
    }

    pub fn select_ir(&mut self, name: &str) -> Result<()> {
        let mut ir_samples = self.ir_loader.load_by_name(name)?;

        // Truncate to max length
        let original_len = ir_samples.len();
        if ir_samples.len() > self.max_ir_samples {
            ir_samples.truncate(self.max_ir_samples);
            info!(
                "IR '{}' truncated from {} to {} samples ({:.1}ms)",
                name,
                original_len,
                self.max_ir_samples,
                self.max_ir_samples as f32 / self.sample_rate as f32 * 1000.0
            );
        }

        // Trim trailing silence
        let ir_samples = Self::trim_silence(&ir_samples);

        info!(
            "Loading IR '{}': {} samples ({:.1}ms)",
            name,
            ir_samples.len(),
            ir_samples.len() as f32 / self.sample_rate as f32 * 1000.0
        );

        // Set IR on convolver
        self.convolver.set_ir(&ir_samples)?;

        Ok(())
    }

    fn trim_silence(ir: &[f32]) -> Vec<f32> {
        // Trim leading silence
        let start = ir.iter().position(|&x| x.abs() > 1e-6).unwrap_or(0);

        // Trim trailing silence
        let mut end = ir.len();
        while end > start && ir[end - 1].abs() < 1e-6 {
            end -= 1;
        }

        if start > 0 || end < ir.len() {
            info!(
                "Trimmed IR: removed {} leading, {} trailing silent samples",
                start,
                ir.len() - end
            );
        }

        ir[start..end].to_vec()
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

    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.output_gain = gain.clamp(0.0, 2.0);
    }

    pub fn gain(&self) -> f32 {
        self.output_gain
    }

    pub fn latency(&self) -> usize {
        self.convolver.latency()
    }

    pub fn convolver_type(&self) -> ConvolverType {
        self.convolver_type
    }

    /// Switch to a different convolver type. Requires re-selecting the IR.
    pub fn set_convolver_type(&mut self, convolver_type: ConvolverType) {
        if self.convolver_type != convolver_type {
            self.convolver_type = convolver_type;
            self.convolver = match convolver_type {
                ConvolverType::Fir => Convolver::new_fir(self.max_ir_samples),
                ConvolverType::TwoStage => Convolver::new_two_stage(),
            };
            info!("Switched to {:?} convolver", convolver_type);
        }
    }

    /// Set maximum IR length in milliseconds
    pub fn set_max_ir_ms(&mut self, max_ms: usize) {
        self.max_ir_samples = (self.sample_rate * max_ms) / 1000;

        // Update FIR convolver if that's what we're using
        if self.convolver_type == ConvolverType::Fir {
            self.convolver = Convolver::new_fir(self.max_ir_samples);
        }

        info!(
            "Max IR length set to {}ms ({} samples)",
            max_ms, self.max_ir_samples
        );
    }
}
