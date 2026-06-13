#![allow(clippy::pedantic, clippy::nursery, clippy::type_complexity)]

//! Real-time path allocation audit.
//!
//! Each test exercises a slice of the engine inside `assert_no_alloc(...)` so
//! that any unexpected heap traffic on the audio thread will panic.
//!
//! Tests that legitimately panic from allocations on the hot loop are marked
//! `#[ignore = "FIXME: ..."]` with a `file:line` pointer to the offender so we
//! have a paper trail of every known issue. Setup-time allocation (constructor
//! work that runs *before* the assert scope) is fine and noted in the per-test
//! comment.

use assert_no_alloc::{AllocDisabler, assert_no_alloc, reset_violation_count, violation_count};
use hound::{WavSpec, WavWriter};

use rustortion_core::amp::chain::{AmplifierChain, DEFAULT_CHAIN_CAPACITY};
use rustortion_core::amp::stages::Stage;
use rustortion_core::amp::stages::clipper::ClipperType;
use rustortion_core::amp::stages::compressor::CompressorStage;
use rustortion_core::amp::stages::delay::DelayStage;
use rustortion_core::amp::stages::eq::{EqStage, NUM_BANDS};
use rustortion_core::amp::stages::filter::{FilterStage, FilterType};
use rustortion_core::amp::stages::level::LevelStage;
use rustortion_core::amp::stages::multiband_saturator::MultibandSaturatorStage;
use rustortion_core::amp::stages::noise_gate::NoiseGateStage;
use rustortion_core::amp::stages::poweramp::{PowerAmpStage, PowerAmpType};
use rustortion_core::amp::stages::preamp::PreampStage;
use rustortion_core::amp::stages::reverb::ReverbStage;
use rustortion_core::amp::stages::tonestack::{ToneStackModel, ToneStackStage};
use rustortion_core::amp::stages::tremolo::TremoloStage;
use rustortion_core::audio::engine::{Engine, EngineHandle, PreparedIr};
use rustortion_core::audio::peak_meter::PeakMeter;
use rustortion_core::audio::rt_drop::{RtDropHandle, RtDropReceiver};
use rustortion_core::audio::samplers::Samplers;
use rustortion_core::ir::cabinet::{ConvolverType, DEFAULT_MAX_IR_MS, IrCabinet};
use rustortion_core::ir::convolver::Convolver;
use rustortion_core::ir::loader::IrLoader;
use rustortion_core::metronome::Metronome;
use rustortion_core::tuner::Tuner;

#[global_allocator]
static A: AllocDisabler = AllocDisabler;

const SAMPLE_RATE: usize = 48_000;
const SAMPLE_RATE_F32: f32 = 48_000.0;
const BUFFER_SIZE: usize = 128;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Run `body` inside `assert_no_alloc` and return the number of violations
/// that were recorded. With the `warn_debug` feature on, `assert_no_alloc`
/// counts allocation attempts instead of aborting — so we can panic with a
/// useful message at the test boundary instead of killing the test binary.
///
/// Always check the returned count and panic if non-zero — calling
/// `assert_no_alloc` directly would silently swallow regressions.
fn check_no_alloc<F: FnOnce()>(body: F) -> u32 {
    reset_violation_count();
    assert_no_alloc(body);
    violation_count()
}

/// Run `engine.process()` `iters` times inside an `assert_no_alloc` scope
/// after one warm-up call. Panics if any allocation was observed.
///
/// Tests using this helper should ensure the message channel is fully drained
/// before calling: the warm-up `process()` happens *outside* the assertion,
/// so any allocation triggered while draining a queued `EngineMessage` (e.g.
/// `PitchShifter::new` from `SetPitchShift`) is invisible to the audit. Send
/// such messages inside the asserted scope instead.
fn assert_engine_alloc_free(engine: &mut Engine, input: &[f32], output: &mut [f32], iters: usize) {
    // Warm up once outside the assertion to amortise any first-call setup
    // (also drains any pending crossbeam message — see doc comment above).
    engine.process(input, output).unwrap();

    let violations = check_no_alloc(|| {
        for _ in 0..iters {
            engine.process(input, output).unwrap();
        }
    });
    assert_eq!(
        violations, 0,
        "engine.process() allocated {violations} time(s) on the RT path"
    );
}

/// Build a plugin-style engine plus the standard input/output buffers.
fn plugin_engine(oversample: f64) -> (Engine, EngineHandle, RtDropReceiver) {
    Engine::new_for_plugin(SAMPLE_RATE, BUFFER_SIZE, None, oversample)
        .expect("engine should construct")
}

/// Plugin-style engine with an IR cabinet attached. No peak meter / tuner /
/// metronome / recorder, so the IR convolver is the only thing under test.
fn plugin_engine_with_ir(
    oversample: f64,
    cabinet: IrCabinet,
) -> (Engine, EngineHandle, RtDropReceiver) {
    Engine::new_for_plugin(SAMPLE_RATE, BUFFER_SIZE, Some(cabinet), oversample)
        .expect("engine should construct")
}

/// Build the full standalone-style engine (tuner + peak meter + metronome
/// present, but disabled). Allows exercising every code path that's gated
/// behind `lightweight = false`.
fn full_engine(oversample: f64, ir_cabinet: Option<IrCabinet>) -> (Engine, EngineHandle) {
    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let samplers = Samplers::new(BUFFER_SIZE, oversample, SAMPLE_RATE).unwrap();
    let metronome = Metronome::new(120.0, SAMPLE_RATE);
    let (engine, handle) = Engine::new(
        tuner,
        samplers,
        ir_cabinet,
        peak_meter,
        metronome,
        RtDropHandle::new().0,
    )
    .unwrap();
    (engine, handle)
}

fn buffers() -> (Vec<f32>, Vec<f32>) {
    (vec![0.5_f32; BUFFER_SIZE], vec![0.0_f32; BUFFER_SIZE])
}

/// Build an in-memory FIR convolver loaded with a tiny synthetic IR. Avoids
/// touching the filesystem so tests stay hermetic.
fn make_fir_convolver() -> Convolver {
    let max_ir_samples = (SAMPLE_RATE * DEFAULT_MAX_IR_MS) / 1000;
    let mut conv = Convolver::new_fir(max_ir_samples);
    let ir: Vec<f32> = (0..256).map(|i| (-(i as f32) / 64.0).exp() * 0.5).collect();
    conv.set_ir(&ir).unwrap();
    conv
}

fn make_two_stage_convolver() -> Convolver {
    let mut conv = Convolver::new_two_stage();
    let ir: Vec<f32> = (0..2048)
        .map(|i| (-(i as f32) / 256.0).exp() * 0.3)
        .collect();
    conv.set_ir(&ir).unwrap();
    conv
}

/// Materialise a tiny WAV on disk so the loader path can also be exercised.
/// Used by the IR-cabinet tests that validate loading via `IrLoader`.
fn write_test_ir(dir: &std::path::Path, name: &str, length: usize) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&path, spec).unwrap();
    for i in 0..length {
        let t = i as f32 / SAMPLE_RATE as f32;
        let decay = (-t * 5.0).exp();
        let s = (440.0 * std::f32::consts::TAU * t).sin() * decay;
        writer.write_sample((s * i16::MAX as f32) as i16).unwrap();
    }
    writer.finalize().unwrap();
    path
}

// ---------------------------------------------------------------------------
// Baseline engine tests (kept from the seed file)
// ---------------------------------------------------------------------------

#[test]
fn engine_process_does_not_allocate_with_empty_chain() {
    // Covers: Engine::process with no chain, no IR, no extras.
    let (mut engine, _handle, _rx) = plugin_engine(1.0);
    let (input, mut output) = buffers();
    assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
}

#[test]
fn engine_process_does_not_allocate_with_chain() {
    // Covers: SetAmpChain warm-up + steady-state processing through one stage.
    let (mut engine, handle, _rx) = plugin_engine(1.0);

    let mut chain = AmplifierChain::new();
    chain.add_stage(Box::new(LevelStage::new(0.5)));
    handle.set_amp_chain(chain);

    let (input, mut output) = buffers();
    assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
}

#[test]
fn engine_process_does_not_allocate_with_oversampling() {
    // Covers: rubato FftFixedInOut up/downsampler hot path at 4x.
    let (mut engine, handle, _rx) = plugin_engine(4.0);

    let mut chain = AmplifierChain::new();
    chain.add_stage(Box::new(LevelStage::new(0.5)));
    handle.set_amp_chain(chain);

    let (input, mut output) = buffers();
    assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
}

// ---------------------------------------------------------------------------
// Per-stage tests
// ---------------------------------------------------------------------------

mod stages {
    //! One test per registered stage type. Each adds the stage via
    //! `EngineHandle::add_stage`, drains the message with one warm-up call,
    //! then asserts the hot loop is allocation-free.

    use super::*;

    fn run_with_stage(stage: Box<dyn Stage>) {
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        handle.add_stage(0, stage);
        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }

    #[test]
    fn preamp_stage_does_not_allocate() {
        // Covers: PreampStage process (asym tanh + interstage LP + DC blocker).
        run_with_stage(Box::new(PreampStage::new(
            5.0,
            0.0,
            ClipperType::Soft,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn compressor_stage_does_not_allocate() {
        // Covers: CompressorStage envelope follower + gain reduction.
        run_with_stage(Box::new(CompressorStage::new(
            1.0,
            50.0,
            -20.0,
            4.0,
            0.0,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn noise_gate_stage_does_not_allocate() {
        // Covers: NoiseGateStage envelope + gate state smoothing.
        run_with_stage(Box::new(NoiseGateStage::new(
            -30.0,
            10.0,
            1.0,
            50.0,
            50.0,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn tonestack_stage_does_not_allocate() {
        // Covers: ToneStackStage 3-band first-order filters + presence shelf.
        run_with_stage(Box::new(ToneStackStage::new(
            ToneStackModel::Modern,
            1.0,
            1.0,
            1.0,
            1.0,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn poweramp_stage_does_not_allocate() {
        // Covers: PowerAmpStage class-A tanh + sag envelope + DC blocker.
        run_with_stage(Box::new(PowerAmpStage::new(
            0.5,
            PowerAmpType::ClassA,
            0.3,
            80.0,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn multiband_saturator_stage_does_not_allocate() {
        // Covers: MultibandSaturatorStage three LR4 bands + per-band saturation.
        run_with_stage(Box::new(MultibandSaturatorStage::new(
            0.5,
            0.5,
            0.5,
            1.0,
            1.0,
            1.0,
            200.0,
            2000.0,
            SAMPLE_RATE_F32,
        )));
    }

    #[test]
    fn level_stage_does_not_allocate() {
        // Covers: LevelStage trivial gain multiply (already in baseline but
        // kept here for symmetry with the other stage entries).
        run_with_stage(Box::new(LevelStage::new(0.5)));
    }

    #[test]
    fn delay_stage_does_not_allocate() {
        // Covers: DelayStage ring buffer read + smoothing. Buffer is
        // pre-allocated to MAX_DELAY_MS in DelayStage::new (init-only, fine).
        run_with_stage(Box::new(DelayStage::new(250.0, 0.4, 0.5, SAMPLE_RATE_F32)));
    }

    #[test]
    fn reverb_stage_does_not_allocate() {
        // Covers: ReverbStage Schroeder bank (8 combs + 4 allpasses).
        run_with_stage(Box::new(ReverbStage::new(0.5, 0.5, 0.3, SAMPLE_RATE_F32)));
    }

    #[test]
    fn eq_stage_does_not_allocate() {
        // Covers: EqStage 16-band cascaded biquads.
        run_with_stage(Box::new(EqStage::new([0.0; NUM_BANDS], SAMPLE_RATE_F32)));
    }

    #[test]
    fn tremolo_stage_does_not_allocate() {
        // Covers: TremoloStage sine LFO + tanh shape morph + depth smoothing.
        run_with_stage(Box::new(TremoloStage::new(5.0, 0.7, 0.5, SAMPLE_RATE_F32)));
    }
}

// ---------------------------------------------------------------------------
// IR cabinet tests
// ---------------------------------------------------------------------------

mod ir_cabinet {
    //! Both convolver implementations exercised end-to-end through the engine.
    //! IRs are synthesised in memory; the convolver allocates its working
    //! buffers in `set_ir`, which runs at construction time (init-only, fine).

    use super::*;

    #[test]
    fn fir_convolver_does_not_allocate() {
        // Covers: IrCabinet + FirConvolver process_block on the hot path.
        // Uses the plugin engine so peak_meter (which does allocate) is
        // out of scope — see extras::peak_meter_does_not_allocate.
        let max_ir_samples = (SAMPLE_RATE * DEFAULT_MAX_IR_MS) / 1000;
        let mut cabinet = IrCabinet::new(ConvolverType::Fir, max_ir_samples);
        cabinet.set_convolver(make_fir_convolver());

        let (mut engine, _handle, _rx) = plugin_engine_with_ir(1.0, cabinet);
        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }

    #[test]
    fn two_stage_fft_convolver_does_not_allocate() {
        // Covers: IrCabinet + TwoStageConvolver (FFT) process_block.
        let max_ir_samples = (SAMPLE_RATE * DEFAULT_MAX_IR_MS) / 1000;
        let mut cabinet = IrCabinet::new(ConvolverType::TwoStage, max_ir_samples);
        cabinet.set_convolver(make_two_stage_convolver());

        let (mut engine, _handle, _rx) = plugin_engine_with_ir(1.0, cabinet);
        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }

    #[test]
    fn ir_cabinet_via_loader_does_not_allocate() {
        // Sanity check: WAV-loaded FIR cabinet behaves the same as the
        // synthesised one. Loading happens before the assert scope.
        let dir = tempfile::tempdir().unwrap();
        let _ = write_test_ir(dir.path(), "tiny.wav", 1024);
        let loader = IrLoader::new(dir.path(), SAMPLE_RATE).unwrap();
        let ir_samples = loader.load_by_name("tiny.wav").unwrap();

        let max_ir_samples = (SAMPLE_RATE * DEFAULT_MAX_IR_MS) / 1000;
        let mut cabinet = IrCabinet::new(ConvolverType::Fir, max_ir_samples);
        let mut convolver = Convolver::new_fir(max_ir_samples);
        convolver.set_ir(&ir_samples).unwrap();
        cabinet.set_convolver(convolver);

        let (mut engine, _handle, _rx) = plugin_engine_with_ir(1.0, cabinet);
        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }
}

// ---------------------------------------------------------------------------
// Engine "extras" wired up via the full Engine::new constructor
// ---------------------------------------------------------------------------

mod extras {
    //! Tuner (disabled), metronome (disabled), peak meter (always on),
    //! input filters, pitch shifter — all wired to the standalone-style
    //! Engine to exercise the lightweight=false code path.

    use super::*;

    #[test]
    fn full_engine_does_not_allocate() {
        // Covers: Engine::new path with tuner (disabled), metronome
        // (disabled), and peak meter (always-on once present).
        //
        // The peak meter does allocate per block; engine.rs gates that call
        // with permit_alloc + FIXME (peak_meter.rs:62). This test asserts the
        // *rest* of the full-engine path is alloc-free given that gate.
        let (mut engine, _handle) = full_engine(1.0, None);
        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }

    #[test]
    fn input_filters_do_not_allocate() {
        // Covers: SetInputFilters + apply_input_filters on the hot path.
        // Uses plugin engine so the peak meter is out of scope (see
        // full_engine_does_not_allocate). FilterStage::new runs at
        // construction time (init-only, fine).
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let hp: Box<dyn Stage> = Box::new(FilterStage::new(
            FilterType::Highpass,
            80.0,
            SAMPLE_RATE_F32,
        ));
        let lp: Box<dyn Stage> = Box::new(FilterStage::new(
            FilterType::Lowpass,
            8000.0,
            SAMPLE_RATE_F32,
        ));
        handle.set_input_filters(Some(hp), Some(lp));

        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }

    #[test]
    fn pitch_shifter_does_not_allocate() {
        // Covers: the RT-side SetPitchShift drain (mem::replace + rt_drop.retire
        // of the previous shifter) AND PitchShifter::process_block (per audio
        // block). The shifter itself is now constructed off the RT thread inside
        // EngineHandle::set_pitch_shift, so we call that *before* the assert
        // scope (GUI-thread work) and assert that draining the message and
        // running the shifter on the hot path allocates nothing.
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        engine.process(&input, &mut output).unwrap();

        // Construct + queue the shifter off the asserted path.
        handle.set_pitch_shift(7);

        let violations = check_no_alloc(|| {
            for _ in 0..16 {
                engine.process(&input, &mut output).unwrap();
            }
        });
        assert_eq!(
            violations, 0,
            "pitch shifter path allocated {violations} time(s)"
        );
    }

    #[test]
    fn recorder_record_block_does_not_allocate() {
        // Covers: Recorder::record_block called from Engine::process under
        // lightweight=false. The recorder reuses a pre-allocated buffer pool
        // (see recorder.rs) so the RT-thread call allocates nothing.
        let (mut engine, handle) = full_engine(1.0, None);
        let tmp = tempfile::tempdir().unwrap();
        handle
            .start_recording(SAMPLE_RATE, tmp.path().to_str().unwrap(), BUFFER_SIZE)
            .unwrap();

        let (input, mut output) = buffers();
        assert_engine_alloc_free(&mut engine, &input, &mut output, 32);
    }
}

// ---------------------------------------------------------------------------
// Message-handler arms exercised *inside* the asserted scope
// ---------------------------------------------------------------------------

mod message_arms {
    //! Each test queues an `EngineMessage` and then drains it via
    //! `engine.handle_messages()` *inside* `check_no_alloc`, so any allocation
    //! or deallocation performed by the message handler on the RT thread is
    //! caught.
    //!
    //! This guards against "warm-up blindness": the shared
    //! `assert_engine_alloc_free` helper drains pending messages in its
    //! pre-assertion warm-up `process()` call, which would hide allocations in
    //! `handle_messages`.
    //!
    //! Building and *sending* the message (`EngineHandle::*`) is GUI-thread work
    //! — it boxes the payload and `try_send`s it — and so is done *before* the
    //! scope. Only the RT-side drain is asserted. We drain via
    //! `handle_messages()` rather than `process()` so the audit is scoped to the
    //! handlers and isn't perturbed by a freshly swapped-in samplers/convolver's
    //! first-`process()` cold-start (a separate, pre-existing cost covered by
    //! the steady-state hot-path tests).

    use super::*;

    /// Queue `setup` messages, drain + warm up outside the audit, then drain
    /// `body`'s queued message under `check_no_alloc`. Returns the violations.
    fn assert_drain_alloc_free(
        engine: &mut Engine,
        input: &[f32],
        output: &mut [f32],
        queue: impl FnOnce(),
    ) -> u32 {
        // Drain any setup messages and amortise first-call work outside the
        // audit.
        engine.process(input, output).unwrap();
        // GUI-thread work: build + send the message under test.
        queue();
        // Assert only the RT-side drain.
        check_no_alloc(|| engine.handle_messages())
    }

    #[test]
    fn add_stage_drain_does_not_allocate() {
        // AddStage inserts into AmplifierChain.stages. The chain reserves
        // DEFAULT_CHAIN_CAPACITY up front so the insert does not reallocate.
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.add_stage(0, Box::new(LevelStage::new(0.5)));
        });
        assert_eq!(
            violations, 0,
            "AddStage drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn add_stage_at_capacity_does_not_allocate() {
        // Fill the chain to its reserved capacity (off-audit), then assert the
        // capacity-crossing AddStage drain neither reallocates nor deallocates:
        // the chain rejects the stage instead of growing its Vec, and the engine
        // retires the rejected box via rt_drop rather than freeing it on RT.
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        for _ in 0..DEFAULT_CHAIN_CAPACITY {
            handle.add_stage(0, Box::new(LevelStage::new(1.0)));
            engine.process(&input, &mut output).unwrap();
        }
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.add_stage(0, Box::new(LevelStage::new(0.5)));
        });
        assert_eq!(
            violations, 0,
            "at-capacity AddStage drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn replace_stage_drain_does_not_allocate() {
        // ReplaceStage swaps the stage in place and retires the old one via
        // rt_drop (a non-allocating try_send).
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        handle.add_stage(0, Box::new(LevelStage::new(0.5)));
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.replace_stage(0, Box::new(LevelStage::new(0.25)));
        });
        assert_eq!(
            violations, 0,
            "ReplaceStage drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn remove_stage_drain_does_not_allocate() {
        // RemoveStage removes from the Vec and retires the old stage via
        // rt_drop. Vec::remove does not reallocate.
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        handle.add_stage(0, Box::new(LevelStage::new(0.5)));
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.remove_stage(0);
        });
        assert_eq!(
            violations, 0,
            "RemoveStage drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn set_input_filters_drain_does_not_allocate() {
        // SetInputFilters now retires the previous filters via rt_drop rather
        // than dropping them on assignment. Install filters first so the
        // asserted swap exercises the retire path with non-None old values.
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        handle.set_input_filters(
            Some(Box::new(FilterStage::new(
                FilterType::Highpass,
                80.0,
                SAMPLE_RATE_F32,
            ))),
            Some(Box::new(FilterStage::new(
                FilterType::Lowpass,
                8000.0,
                SAMPLE_RATE_F32,
            ))),
        );
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.set_input_filters(
                Some(Box::new(FilterStage::new(
                    FilterType::Highpass,
                    100.0,
                    SAMPLE_RATE_F32,
                ))),
                Some(Box::new(FilterStage::new(
                    FilterType::Lowpass,
                    6000.0,
                    SAMPLE_RATE_F32,
                ))),
            );
        });
        assert_eq!(
            violations, 0,
            "SetInputFilters drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn set_samplers_drain_does_not_allocate() {
        // SetSamplers swaps the boxed samplers and retires the old box directly
        // (no Box::new type-erasure on the RT thread).
        let (mut engine, handle, _rx) = plugin_engine(1.0);
        let (input, mut output) = buffers();
        let samplers = Samplers::new(BUFFER_SIZE, 2.0, SAMPLE_RATE).unwrap();
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.set_samplers(samplers);
        });
        assert_eq!(
            violations, 0,
            "SetSamplers drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn swap_ir_convolver_drain_does_not_allocate() {
        // SwapIrConvolver swaps the boxed convolver in place and retires the
        // whole PreparedIr (old convolver + name String) via rt_drop.
        let max_ir_samples = (SAMPLE_RATE * DEFAULT_MAX_IR_MS) / 1000;
        let mut cabinet = IrCabinet::new(ConvolverType::Fir, max_ir_samples);
        cabinet.set_convolver(make_fir_convolver());
        let (mut engine, handle, _rx) = plugin_engine_with_ir(1.0, cabinet);
        let (input, mut output) = buffers();
        let prepared = PreparedIr {
            name: "swap-test".to_string(),
            convolver: Box::new(make_fir_convolver()),
        };
        let violations = assert_drain_alloc_free(&mut engine, &input, &mut output, || {
            handle.swap_ir_convolver(prepared);
        });
        assert_eq!(
            violations, 0,
            "SwapIrConvolver drain allocated {violations} time(s)"
        );
    }

    #[test]
    fn set_tuner_enabled_drain_does_not_allocate() {
        // SetTunerEnabled(true) flips a bool; the subsequent tuner.process
        // appends into a pre-allocated 4096-sample buffer. Stay well under the
        // detection threshold so no AMDF run (which allocates a readout) fires.
        // This one drives process() (not just the drain) to also cover the
        // tuner buffer-growth path.
        let (mut engine, handle) = full_engine(1.0, None);
        let (input, mut output) = buffers();
        engine.process(&input, &mut output).unwrap();

        let violations = check_no_alloc(|| {
            handle.set_tuner_enabled(true);
            for _ in 0..8 {
                engine.process(&input, &mut output).unwrap();
            }
        });
        assert_eq!(
            violations, 0,
            "SetTunerEnabled drain allocated {violations} time(s)"
        );
    }
}

// ---------------------------------------------------------------------------
// Metronome
// ---------------------------------------------------------------------------

#[test]
fn metronome_processing_does_not_allocate() {
    // Covers: Metronome::process_block, the per-block work behind
    // Engine::process_metronome (driven on the RT thread for the standalone
    // metronome output port).
    let mut metronome = Metronome::new(120.0, SAMPLE_RATE);
    metronome.toggle_metronome(); // enable
    let mut output = vec![0.0_f32; BUFFER_SIZE];

    let violations = check_no_alloc(|| {
        for _ in 0..32 {
            metronome.process_block(&mut output);
        }
    });
    assert_eq!(
        violations, 0,
        "metronome processing allocated {violations} time(s)"
    );
}
