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

/// Regression tests for the plugin-side engine: variable host block sizes,
/// reported latency, and the master-bus non-finite guard.
mod variable_block_and_guards {
    use rustortion_core::amp::chain::AmplifierChain;
    use rustortion_core::amp::stages::delay::DelayStage;
    use rustortion_core::amp::stages::level::LevelStage;
    use rustortion_core::amp::stages::reverb::ReverbStage;
    use rustortion_core::audio::engine::{Engine, PITCH_SHIFTER_LATENCY_FRAMES, PreparedIr};
    use rustortion_core::audio::pitch_shifter::PitchShifter;
    use rustortion_core::ir::cabinet::{ConvolverType, IrCabinet};
    use rustortion_core::ir::convolver::Convolver;

    const SR: usize = 48_000;
    const MAX_BLOCK: usize = 512;

    /// Before the fix, `Engine::process` returned `Err` for any block shorter
    /// than the size the samplers were constructed with, and the plugin's error
    /// path left the host buffer untouched — i.e. the dry, unprocessed input
    /// played through at full level. Hosts deliver short blocks routinely
    /// (loop boundaries, transport start, offline bounce).
    #[test]
    fn short_blocks_with_oversampling_produce_processed_audio_not_dry() {
        let (mut engine, handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 2.0).expect("engine should construct");

        let mut chain = AmplifierChain::new();
        chain.add_stage(Box::new(LevelStage::new(0.5)));
        handle.set_amp_chain(chain);

        // Neither the constructed maximum nor a divisor of the resampler chunk.
        let block = 137;
        let input = vec![1.0_f32; block];
        let mut output = vec![0.0_f32; block];

        for _ in 0..64 {
            engine
                .process(&input, &mut output)
                .expect("a short block must not fail");
        }

        let mean = output.iter().sum::<f32>() / block as f32;
        assert!(
            (mean - 0.5).abs() < 0.01,
            "expected the processed signal (DC 0.5), got {mean} — 1.0 would mean the dry input leaked"
        );
    }

    /// Blocks are accumulated into a FIFO and resampled in fixed chunks. Zero
    /// padding a short block would inject silence into the resampler state and
    /// tear the signal at every boundary, so feed deliberately ragged block
    /// sizes and require the output to be the input delayed by exactly the
    /// latency the engine reports.
    #[test]
    fn ragged_block_sizes_preserve_the_signal_and_match_reported_latency() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 2.0).expect("engine should construct");

        let total = 24_000;
        let input: Vec<f32> = (0..total)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR as f32).sin() * 0.5)
            .collect();

        let sizes = [512, 1, 128, 7, 480, 63, 256, 33];
        let mut out: Vec<f32> = Vec::with_capacity(total);
        let mut pos = 0;
        let mut k = 0;
        while pos < total {
            let n = sizes[k % sizes.len()].min(total - pos);
            k += 1;
            let block = input[pos..pos + n].to_vec();
            let mut buf = vec![0.0_f32; n];
            engine
                .process(&block, &mut buf)
                .expect("process must succeed");
            out.extend_from_slice(&buf);
            pos += n;
        }

        assert_eq!(out.len(), total);

        // The first block is a clean multiple of the chunk and takes the fast
        // path; the second (1 frame) engages the FIFO. Everything from there on
        // is aligned to the engaged latency, so that is what to compare against.
        let latency = engine.latency_frames();
        assert!(engine.fifo_engaged(), "ragged blocks must engage the FIFO");

        // Skip the startup transient, then compare against the delayed input.
        let start = latency + 2_000;
        let end = total - 1_000;
        let max_err = (start..end)
            .map(|i| (out[i] - input[i - latency]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_err < 0.02,
            "max abs error {max_err} at reported latency {latency} — either the signal is \
             corrupted across block boundaries or the reported latency is wrong"
        );
    }

    /// Feed a sine through the engine in `block` sized chunks and return the
    /// output, so the fast path and the engaged path can be compared with the
    /// same technique.
    fn run_blocks(engine: &mut Engine, input: &[f32], block: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(input.len());
        for chunk in input.chunks(block) {
            let mut buf = vec![0.0_f32; chunk.len()];
            engine
                .process(chunk, &mut buf)
                .expect("process must succeed");
            out.extend_from_slice(&buf);
        }
        out
    }

    fn sine(total: usize) -> Vec<f32> {
        (0..total)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR as f32).sin() * 0.5)
            .collect()
    }

    /// A fixed-block host must never pay for the FIFO. Every standard buffer
    /// size is an exact multiple of the 256-frame chunk, so the fast path holds
    /// for the whole session.
    #[test]
    fn fixed_size_blocks_never_engage_the_fifo() {
        for max_block in [512_usize, 2048] {
            for block in [64_usize, 128, 256, 512] {
                if block > max_block {
                    continue;
                }
                let (mut engine, _handle, _rx) = Engine::new_for_plugin(SR, max_block, None, 2.0)
                    .expect("engine should construct");
                let fast_latency = engine.latency_frames();

                let input = vec![0.25_f32; block];
                let mut output = vec![0.0_f32; block];
                for _ in 0..64 {
                    engine.process(&input, &mut output).unwrap();
                }

                assert!(
                    !engine.fifo_engaged(),
                    "max_block {max_block}, block {block}: fixed blocks must not engage the FIFO"
                );
                assert_eq!(
                    engine.latency_frames(),
                    64,
                    "max_block {max_block}, block {block}: fast path is resampler delay only"
                );
                assert_eq!(
                    engine.latency_frames(),
                    fast_latency,
                    "latency must not drift"
                );

                // And it is genuinely cheaper than the engaged figure.
                let (mut engaged, _h, _r) = Engine::new_for_plugin(SR, max_block, None, 2.0)
                    .expect("engine should construct");
                let ragged_in = vec![0.0_f32; 7];
                let mut ragged_out = vec![0.0_f32; 7];
                engaged.process(&ragged_in, &mut ragged_out).unwrap();
                assert!(
                    fast_latency < engaged.latency_frames(),
                    "max_block {max_block}: fast path {fast_latency} should beat engaged {}",
                    engaged.latency_frames()
                );
            }
        }
    }

    /// Audio through the fast path must be the input delayed by exactly the
    /// (small) latency it reports.
    #[test]
    fn fast_path_audio_is_correct() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 2.0).expect("engine should construct");

        // A whole number of blocks: a short final block would engage the FIFO
        // and defeat the point of the test.
        let total = 24_576;
        let input = sine(total);
        let out = run_blocks(&mut engine, &input, 512);

        assert!(!engine.fifo_engaged(), "512 is a multiple of the chunk");
        let latency = engine.latency_frames();
        let max_err = (latency + 2_000..total - 1_000)
            .map(|i| (out[i] - input[i - latency]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_err < 0.02,
            "fast path max abs error {max_err} at latency {latency}"
        );
    }

    /// The first block the fast path cannot take engages the FIFO, latency
    /// grows to the engaged figure, and the audio stays correct against it.
    #[test]
    fn ragged_block_engages_the_fifo_and_audio_stays_correct() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 2.0).expect("engine should construct");
        let fast_latency = engine.latency_frames();

        // One ragged block up front: engage immediately so the whole run is in
        // the engaged regime.
        let seed_in = vec![0.0_f32; 100];
        let mut seed_out = vec![0.0_f32; 100];
        engine.process(&seed_in, &mut seed_out).unwrap();

        assert!(engine.fifo_engaged(), "a 100-frame block must engage");
        let engaged_latency = engine.latency_frames();
        assert!(
            engaged_latency > fast_latency,
            "engaging must increase latency: {fast_latency} -> {engaged_latency}"
        );
        assert_eq!(
            engaged_latency, 640,
            "512 max block: 64 frames of resampler delay + a 576 frame prefill"
        );

        let total = 24_000;
        let input = sine(total);
        let out = run_blocks(&mut engine, &input, 300);

        let max_err = (engaged_latency + 2_000..total - 1_000)
            .map(|i| (out[i] - input[i - engaged_latency]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_err < 0.02,
            "engaged path max abs error {max_err} at latency {engaged_latency}"
        );
    }

    /// Engagement is sticky: clean blocks afterwards must not silently drop the
    /// latency back, which would re-report to the host and restart playback.
    #[test]
    fn once_engaged_the_fifo_stays_engaged() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 2.0).expect("engine should construct");

        let ragged_in = vec![0.0_f32; 37];
        let mut ragged_out = vec![0.0_f32; 37];
        engine.process(&ragged_in, &mut ragged_out).unwrap();
        let engaged_latency = engine.latency_frames();
        assert!(engine.fifo_engaged());

        let input = vec![0.1_f32; 512];
        let mut output = vec![0.0_f32; 512];
        for _ in 0..64 {
            engine.process(&input, &mut output).unwrap();
            assert!(engine.fifo_engaged(), "must never disengage");
            assert_eq!(
                engine.latency_frames(),
                engaged_latency,
                "latency must not drop back once engaged"
            );
        }
    }

    /// A `max_buffer_size` that is not a multiple of the chunk must engage the
    /// FIFO on the first block rather than misbehaving.
    #[test]
    fn odd_max_block_size_engages_gracefully() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, 300, None, 2.0).expect("engine should construct");
        let input = vec![0.25_f32; 300];
        let mut output = vec![0.0_f32; 300];
        engine.process(&input, &mut output).unwrap();
        assert!(engine.fifo_engaged(), "300 is not a multiple of 64");
        assert!(output.iter().all(|s| s.is_finite()));
    }

    /// The engine's latency must reflect the pitch shifter, which is installed
    /// and removed at runtime.
    #[test]
    fn latency_tracks_the_pitch_shifter() {
        let (mut engine, handle, _rx) =
            Engine::new_for_plugin(SR, MAX_BLOCK, None, 1.0).expect("engine should construct");
        let mut buf = vec![0.0_f32; 64];
        let input = vec![0.0_f32; 64];

        engine.process(&input, &mut buf).unwrap();
        assert_eq!(engine.latency_frames(), 0, "1x, no shifter: no latency");

        handle.set_pitch_shift(7);
        engine.process(&input, &mut buf).unwrap();
        assert_eq!(engine.latency_frames(), PITCH_SHIFTER_LATENCY_FRAMES);

        handle.set_pitch_shift(0);
        engine.process(&input, &mut buf).unwrap();
        assert_eq!(engine.latency_frames(), 0);
    }

    /// Pins `PITCH_SHIFTER_LATENCY_FRAMES` to the shifter's measured group
    /// delay: an impulse in, energy centroid out.
    #[test]
    fn pitch_shifter_latency_matches_constant() {
        let mut shifter = PitchShifter::new(0.0);
        let mut buf = vec![0.0_f32; 16_384];
        buf[0] = 1.0;
        shifter.process_block(&mut buf);

        let energy: f32 = buf.iter().map(|v| v * v).sum();
        assert!(energy > 0.0, "impulse produced no output");
        let centroid: f32 = buf
            .iter()
            .enumerate()
            .map(|(i, v)| i as f32 * v * v)
            .sum::<f32>()
            / energy;

        assert!(
            (centroid - PITCH_SHIFTER_LATENCY_FRAMES as f32).abs() < 8.0,
            "measured pitch shifter delay {centroid} != PITCH_SHIFTER_LATENCY_FRAMES \
             ({PITCH_SHIFTER_LATENCY_FRAMES})"
        );
    }

    // -----------------------------------------------------------------
    // Tail reporting: must reflect what is actually in the chain, since
    // presets frequently run with no IR cabinet loaded.
    // -----------------------------------------------------------------

    fn engine_with_chain(stages: Vec<Box<dyn rustortion_core::amp::stages::Stage>>) -> Engine {
        // `_rx` is dropped with the helper: `rt_drop` retirement then just
        // fails to send and the box is freed here, which is fine off the RT
        // thread and keeps the helper's signature simple.
        let (mut engine, handle, _rx) =
            Engine::new_for_plugin(SR, 128, None, 1.0).expect("engine should construct");
        let mut chain = AmplifierChain::new();
        for stage in stages {
            chain.add_stage(stage);
        }
        handle.set_amp_chain(chain);
        // The chain is applied (and the tail recomputed) when messages drain.
        let input = vec![0.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        engine.process(&input, &mut output).unwrap();
        engine
    }

    #[test]
    fn clean_chain_with_no_ir_has_no_tail() {
        let engine = engine_with_chain(vec![Box::new(LevelStage::new(0.5))]);
        assert_eq!(
            engine.tail_seconds(),
            0.0,
            "nothing in this chain rings, so the host must be free to idle"
        );
    }

    #[test]
    fn delay_rings_without_any_ir() {
        // 500 ms at 0.5 feedback decays to -60 dB after ln(0.001)/ln(0.5)
        // = 9.97 repeats, i.e. ~4.98 s.
        let engine = engine_with_chain(vec![Box::new(DelayStage::new(500.0, 0.5, 1.0, SR as f32))]);
        let tail = engine.tail_seconds();
        assert!(
            (tail - 4.98).abs() < 0.05,
            "expected ~4.98 s of delay tail with no cab loaded, got {tail}"
        );
    }

    #[test]
    fn delay_with_no_wet_signal_has_no_tail() {
        let engine = engine_with_chain(vec![Box::new(DelayStage::new(500.0, 0.9, 0.0, SR as f32))]);
        assert_eq!(engine.tail_seconds(), 0.0, "a fully dry delay cannot ring");
    }

    #[test]
    fn reverb_rings_without_any_ir() {
        // Freeverb comb feedback = 0.5 * 0.28 + 0.7 = 0.84; RT60 of the longest
        // comb (1617 frames @ 44.1 kHz) is 0.03667 * ln(0.001)/ln(0.84) ~ 1.45 s.
        let engine = engine_with_chain(vec![Box::new(ReverbStage::new(0.5, 0.5, 0.5, SR as f32))]);
        let tail = engine.tail_seconds();
        assert!(
            (tail - 1.45).abs() < 0.05,
            "expected ~1.45 s of reverb tail with no cab loaded, got {tail}"
        );
    }

    #[test]
    fn tail_is_the_max_over_active_sources() {
        let engine = engine_with_chain(vec![
            Box::new(ReverbStage::new(0.5, 0.5, 0.5, SR as f32)),
            Box::new(DelayStage::new(500.0, 0.5, 1.0, SR as f32)),
            Box::new(LevelStage::new(0.5)),
        ]);
        let tail = engine.tail_seconds();
        assert!(
            (tail - 4.98).abs() < 0.05,
            "the longest ringing stage should win, got {tail}"
        );
    }

    #[test]
    fn tail_tracks_parameter_changes() {
        let (mut engine, handle, _rx) =
            Engine::new_for_plugin(SR, 128, None, 1.0).expect("engine should construct");
        let mut chain = AmplifierChain::new();
        chain.add_stage(Box::new(DelayStage::new(100.0, 0.5, 1.0, SR as f32)));
        handle.set_amp_chain(chain);

        let input = vec![0.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        engine.process(&input, &mut output).unwrap();
        let before = engine.tail_seconds();

        handle.set_parameter(0, "delay_time", 1000.0);
        engine.process(&input, &mut output).unwrap();
        let after = engine.tail_seconds();

        assert!(
            after > before * 9.0,
            "a 10x longer delay time should stretch the tail: {before} -> {after}"
        );
    }

    /// An IR contributes a tail only while one is actually loaded — the case
    /// `has_ir` was added for.
    #[test]
    fn ir_contributes_a_tail_only_when_one_is_loaded() {
        let cabinet = IrCabinet::new(ConvolverType::Fir, 4096);
        let (mut engine, handle, _rx) =
            Engine::new_for_plugin(SR, 128, Some(cabinet), 1.0).expect("engine should construct");

        let input = vec![0.0_f32; 64];
        let mut output = vec![0.0_f32; 64];

        assert_eq!(
            engine.tail_seconds(),
            0.0,
            "an empty cabinet must not claim a tail"
        );

        let latency_without_ir = engine.latency_frames();

        let mut convolver = Convolver::new_fir(4096);
        convolver
            .set_ir(
                &(0..1024)
                    .map(|i| (-(i as f32) / 128.0).exp())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        handle.swap_ir_convolver(PreparedIr {
            name: "test".to_string(),
            convolver: Box::new(convolver),
        });
        engine.process(&input, &mut output).unwrap();
        assert!(
            engine.tail_seconds() > 0.0,
            "a loaded IR must contribute a tail"
        );

        // Tail and latency are independent: the FIR convolver is direct form,
        // so an IR rings but does not delay.
        assert_eq!(
            engine.latency_frames(),
            latency_without_ir,
            "loading an IR must not change the reported latency"
        );

        handle.clear_ir();
        engine.process(&input, &mut output).unwrap();
        assert_eq!(
            engine.tail_seconds(),
            0.0,
            "clearing the IR must drop the tail back to zero"
        );
    }

    /// The plugin hardcodes the direct-form FIR convolver, which puts the first
    /// IR tap on the very sample that produced it — no latency to report.
    #[test]
    fn fir_cabinet_adds_no_latency() {
        let mut cabinet = IrCabinet::new(ConvolverType::Fir, 4096);
        let mut convolver = Convolver::new_fir(4096);
        // First coefficient is the largest, so a zero-latency convolver puts
        // the peak at sample 0.
        convolver
            .set_ir(
                &(0..512)
                    .map(|i| (-(i as f32) / 64.0).exp())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        cabinet.set_convolver(convolver);
        cabinet.set_gain(1.0);

        let mut impulse = vec![0.0_f32; 512];
        impulse[0] = 1.0;
        cabinet.process_block(&mut impulse);

        assert!(
            impulse[0] > 0.99,
            "the first IR tap must land on sample 0 (got {}), i.e. zero latency",
            impulse[0]
        );
    }

    /// A NaN or infinity reaching the master bus must be replaced with silence
    /// and flagged, rather than being handed to the host (where it can wedge
    /// downstream feedback state until the session is reloaded).
    #[test]
    fn nonfinite_samples_are_replaced_with_silence() {
        let (mut engine, _handle, _rx) =
            Engine::new_for_plugin(SR, 128, None, 1.0).expect("engine should construct");

        let mut input = vec![0.25_f32; 128];
        input[10] = f32::NAN;
        input[20] = f32::INFINITY;
        input[30] = f32::NEG_INFINITY;
        let mut output = vec![0.0_f32; 128];

        engine.process(&input, &mut output).unwrap();

        assert!(
            output.iter().all(|s| s.is_finite()),
            "non-finite sample reached the output"
        );
        assert_eq!(output[10], 0.0);
        assert_eq!(output[20], 0.0);
        assert_eq!(output[30], 0.0);
        assert!(
            (output[0] - 0.25).abs() < 1e-6,
            "finite samples must be untouched"
        );

        assert!(
            engine.take_nonfinite_seen(),
            "the guard should flag the event"
        );
        assert!(
            !engine.take_nonfinite_seen(),
            "the flag should clear on read"
        );
    }
}
