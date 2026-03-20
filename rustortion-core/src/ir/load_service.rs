use std::collections::HashMap;
use std::thread;

use crossbeam::channel::{Sender, unbounded};
use log::{debug, error, info};

use crate::audio::engine::{EngineHandle, PreparedIr};
use crate::ir::cabinet::ConvolverType;
use crate::ir::convolver::Convolver;
use crate::ir::loader::IrLoader;

enum IrRequest {
    /// Load an IR and send the built convolver to the engine.
    Load(String),
    /// Load an IR into the cache only (no convolver sent).
    Preload(String),
    /// Shut down the background thread.
    Shutdown,
}

/// Handle held by the `Manager` to send IR load requests.
pub struct IrLoadHandle {
    request_tx: Sender<IrRequest>,
    thread: Option<thread::JoinHandle<()>>,
}

impl IrLoadHandle {
    /// Request loading an IR by name and sending the built convolver to the engine.
    pub fn request_load(&self, name: &str) {
        if let Err(e) = self.request_tx.send(IrRequest::Load(name.to_owned())) {
            error!("Failed to send IR load request: {e}");
        }
    }

    /// Preload IR coefficients into the cache without sending to the engine.
    pub fn preload(&self, name: &str) {
        if let Err(e) = self.request_tx.send(IrRequest::Preload(name.to_owned())) {
            error!("Failed to send IR preload request: {e}");
        }
    }
}

impl Drop for IrLoadHandle {
    fn drop(&mut self) {
        let _ = self.request_tx.send(IrRequest::Shutdown);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Trim leading and trailing silence from IR samples.
fn trim_silence(ir: &[f32]) -> &[f32] {
    let start = ir.iter().position(|&x| x.abs() > 1e-6).unwrap_or(0);

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

    &ir[start..end]
}

/// Build a `Convolver` from IR coefficients.
fn build_convolver(
    coefficients: &[f32],
    convolver_type: ConvolverType,
    max_ir_samples: usize,
) -> Convolver {
    let mut convolver = match convolver_type {
        ConvolverType::Fir => Convolver::new_fir(max_ir_samples),
        ConvolverType::TwoStage => Convolver::new_two_stage(),
    };

    if let Err(e) = convolver.set_ir(coefficients) {
        error!("Failed to set IR on convolver: {e}");
    }

    convolver
}

/// Spawn the IR load service on a background thread.
///
/// The service receives IR load requests, loads/resamples WAV files via `IrLoader`,
/// caches the coefficients, builds a `Convolver`, and sends it to the engine as an
/// `EngineMessage::SwapIrConvolver`.
///
pub fn spawn(
    ir_loader: IrLoader,
    engine_handle: EngineHandle,
    sample_rate: usize,
    max_ir_ms: usize,
    convolver_type: ConvolverType,
) -> IrLoadHandle {
    let (request_tx, request_rx) = unbounded::<IrRequest>();
    let max_ir_samples = (sample_rate * max_ir_ms) / 1000;

    let thread = thread::Builder::new()
        .name("ir-load-service".into())
        .spawn(move || {
            let mut cache: HashMap<String, Vec<f32>> = HashMap::new();

            while let Ok(request) = request_rx.recv() {
                match request {
                    IrRequest::Load(name) => {
                        if !cache.contains_key(&name)
                            && !load_and_cache(
                                &ir_loader,
                                &name,
                                max_ir_samples,
                                sample_rate,
                                &mut cache,
                            )
                        {
                            continue;
                        }

                        let coefficients = cache.get(&name).unwrap();
                        let convolver =
                            build_convolver(coefficients, convolver_type, max_ir_samples);
                        let prepared = PreparedIr {
                            name: name.clone(),
                            convolver,
                        };

                        engine_handle.swap_ir_convolver(prepared);

                        debug!("IR '{name}' loaded and sent to engine");
                    }
                    IrRequest::Preload(name) => {
                        if cache.contains_key(&name) {
                            debug!("IR '{name}' already cached, skipping preload");
                            continue;
                        }
                        load_and_cache(&ir_loader, &name, max_ir_samples, sample_rate, &mut cache);
                        debug!("IR '{name}' preloaded into cache");
                    }
                    IrRequest::Shutdown => {
                        debug!("IR load service shutting down");
                        break;
                    }
                }
            }
        })
        .expect("Failed to spawn IR load service thread");

    IrLoadHandle {
        request_tx,
        thread: Some(thread),
    }
}

/// Load an IR by name, process it (truncate, trim silence), and insert into the cache.
/// Returns `true` on success.
fn load_and_cache(
    loader: &IrLoader,
    name: &str,
    max_ir_samples: usize,
    sample_rate: usize,
    cache: &mut HashMap<String, Vec<f32>>,
) -> bool {
    match loader.load_by_name(name) {
        Ok(mut samples) => {
            let original_len = samples.len();
            if samples.len() > max_ir_samples {
                samples.truncate(max_ir_samples);
                info!(
                    "IR '{}' truncated from {} to {} samples ({:.1}ms)",
                    name,
                    original_len,
                    max_ir_samples,
                    max_ir_samples as f32 / sample_rate as f32 * 1000.0
                );
            }

            let trimmed = trim_silence(&samples);
            debug!(
                "Loading IR '{}': {} samples ({:.1}ms)",
                name,
                trimmed.len(),
                trimmed.len() as f32 / sample_rate as f32 * 1000.0
            );

            cache.insert(name.to_owned(), trimmed.to_vec());
            true
        }
        Err(e) => {
            error!("Failed to load IR '{name}': {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_silence_removes_leading_trailing() {
        let ir = vec![0.0, 0.0, 1.0, 0.5, 0.0, 0.0];
        let trimmed = trim_silence(&ir);
        assert_eq!(trimmed, &[1.0, 0.5]);
    }

    #[test]
    fn test_trim_silence_no_silence() {
        let ir = vec![1.0, 0.5, 0.25];
        let trimmed = trim_silence(&ir);
        assert_eq!(trimmed, &[1.0, 0.5, 0.25]);
    }

    #[test]
    fn test_trim_silence_all_silence() {
        let ir = vec![0.0, 0.0, 0.0];
        let trimmed = trim_silence(&ir);
        assert!(trimmed.is_empty());
    }

    #[test]
    fn test_build_convolver_fir() {
        let coefficients = vec![1.0, 0.5, 0.25];
        let mut convolver = build_convolver(&coefficients, ConvolverType::Fir, 1024);
        // Verify it processes correctly (impulse response)
        let y0 = convolver.process_sample(1.0);
        let y1 = convolver.process_sample(0.0);
        assert!((y0 - 1.0).abs() < 1e-6);
        assert!((y1 - 0.5).abs() < 1e-6);
    }
}
