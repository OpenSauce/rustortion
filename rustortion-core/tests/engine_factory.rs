#![allow(clippy::pedantic, clippy::nursery)]

use rustortion_core::audio::engine::Engine;
use rustortion_core::ir::cabinet::{ConvolverType, IrCabinet};

#[test]
fn new_for_plugin_creates_engine_and_handle() {
    let (mut engine, _handle, _rt_drop_rx) =
        Engine::new_for_plugin(48_000, 128, None, 1.0).expect("Engine creation should succeed");

    // Engine should process silence without errors
    let input = [0.0f32; 128];
    let mut output = [0.0f32; 128];
    engine
        .process(&input, &mut output)
        .expect("process should succeed");

    // Output should be silence (no stages in chain)
    assert!(output.iter().all(|&s| s == 0.0));
}

#[test]
fn new_for_plugin_with_ir_cabinet() {
    let cabinet = IrCabinet::new(ConvolverType::Fir, 48_000 * 500 / 1000);
    let (mut engine, _handle, _rx) = Engine::new_for_plugin(48_000, 128, Some(cabinet), 1.0)
        .expect("Engine creation should succeed");

    let input = vec![1.0f32; 128];
    let mut output = vec![0.0f32; 128];
    engine
        .process(&input, &mut output)
        .expect("process should succeed");
}
