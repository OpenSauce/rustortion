use rustortion_core::audio::engine::{EngineHandle, PreparedIr};
use rustortion_core::ir::convolver::Convolver;
use rustortion_core::ir::loader::IrLoader;

/// Load an IR by name, truncate to 35ms (cab sim only, no room tail),
/// and swap it into the engine. Shared between `PluginBackend::set_ir`
/// and `do_load_preset`.
pub fn load_and_set_ir(handle: &EngineHandle, loader: &IrLoader, name: &str, sample_rate: f32) {
    match loader.load_by_name(name) {
        Ok(ir_samples) => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let max_ir_len = (sample_rate * 35.0 / 1000.0) as usize;
            let truncated_len = ir_samples.len().min(max_ir_len);
            let mut convolver = Convolver::new_fir(truncated_len);
            if let Err(e) = convolver.set_ir(&ir_samples[..truncated_len]) {
                log::error!("Failed to set IR: {e}");
            } else {
                handle.swap_ir_convolver(PreparedIr {
                    name: name.to_string(),
                    convolver,
                });
            }
        }
        Err(e) => log::error!("Failed to load IR '{name}': {e}"),
    }
}

/// Load an IR from raw WAV bytes, truncate to 35ms, and swap into engine.
pub fn load_and_set_ir_from_bytes(
    handle: &EngineHandle,
    loader: &IrLoader,
    name: &str,
    bytes: &[u8],
    sample_rate: f32,
) {
    match loader.load_ir_from_bytes(bytes) {
        Ok(ir_samples) => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let max_ir_len = (sample_rate * 35.0 / 1000.0) as usize;
            let truncated_len = ir_samples.len().min(max_ir_len);
            let mut convolver = Convolver::new_fir(truncated_len);
            if let Err(e) = convolver.set_ir(&ir_samples[..truncated_len]) {
                log::error!("Failed to set IR: {e}");
            } else {
                handle.swap_ir_convolver(PreparedIr {
                    name: name.to_string(),
                    convolver,
                });
            }
        }
        Err(e) => log::error!("Failed to load embedded IR '{name}': {e}"),
    }
}
