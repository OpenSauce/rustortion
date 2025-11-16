use anyhow::Result;
use rustortion::audio::engine::Engine;
use rustortion::audio::peak_meter::PeakMeter;
use rustortion::audio::samplers::Samplers;
use rustortion::sim::chain::AmplifierChain;
use rustortion::sim::stages::level::LevelStage;
use rustortion::sim::tuner::Tuner;

#[test]
fn engine_processes_non_zero_signal() -> Result<()> {
    const SAMPLE_RATE: usize = 48_000;
    const BUFFER_SIZE: usize = 128;
    const OVERSAMPLE_FACTOR: f64 = 1.0;

    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let samplers = Samplers::new(BUFFER_SIZE, OVERSAMPLE_FACTOR)?;
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let (mut engine, _) = Engine::new(tuner, samplers, None, peak_meter)?;

    let input = vec![0.5f32; BUFFER_SIZE];
    let mut output = vec![0.0f32; BUFFER_SIZE];

    for _ in 0..10 {
        engine.process(&input, &mut output)?;
    }

    assert!(output.iter().any(|&x| x != 0.0), "expected non-zero output");

    Ok(())
}

#[test]
fn engine_handles_buffer_size_change() -> Result<()> {
    const SAMPLE_RATE: usize = 48000;
    const INITIAL_BUFFER_SIZE: usize = 128;
    const NEW_BUFFER_SIZE: usize = 256;
    const OVERSAMPLE_FACTOR: f64 = 1.0;

    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let samplers = Samplers::new(INITIAL_BUFFER_SIZE, OVERSAMPLE_FACTOR)?;
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let (mut engine, _) = Engine::new(tuner, samplers, None, peak_meter)?;

    let input = vec![0.5f32; INITIAL_BUFFER_SIZE];
    let mut output = vec![0.0f32; INITIAL_BUFFER_SIZE];
    engine.process(&input, &mut output)?;

    assert_eq!(
        output.len(),
        INITIAL_BUFFER_SIZE,
        "output length should match initial buffer size"
    );

    engine.update_buffer_size(NEW_BUFFER_SIZE)?;

    let input = vec![0.5f32; NEW_BUFFER_SIZE];
    let mut output = vec![0.0f32; NEW_BUFFER_SIZE];
    engine.process(&input, &mut output)?;

    assert!(
        output.iter().any(|&x| x != 0.0),
        "expected non-zero output after buffer size change"
    );
    assert_eq!(
        output.len(),
        NEW_BUFFER_SIZE,
        "output length should match new buffer size"
    );

    Ok(())
}

#[test]
fn engine_rejects_mismatched_buffer_sizes() -> Result<()> {
    const SAMPLE_RATE: usize = 48000;
    const BUFFER_SIZE: usize = 128;
    const OVERSAMPLE_FACTOR: f64 = 1.0;

    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let samplers = Samplers::new(BUFFER_SIZE, OVERSAMPLE_FACTOR)?;
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let (mut engine, _) = Engine::new(tuner, samplers, None, peak_meter)?;

    let small_input = vec![0.5f32; BUFFER_SIZE / 2];
    let mut small_output = vec![0.0f32; BUFFER_SIZE / 2];
    assert!(
        engine.process(&small_input, &mut small_output).is_err(),
        "expected error when input buffer size is smaller than expected"
    );

    let large_input = vec![0.5f32; BUFFER_SIZE * 2];
    let mut large_output = vec![0.0f32; BUFFER_SIZE * 2];
    assert!(
        engine.process(&large_input, &mut large_output).is_err(),
        "expected error when input buffer size is larger than expected"
    );

    Ok(())
}

#[test]
fn engine_applies_amp_chain() -> Result<()> {
    const SAMPLE_RATE: usize = 48000;
    const BUFFER_SIZE: usize = 128;
    const OVERSAMPLE_FACTOR: f64 = 1.0;

    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let samplers = Samplers::new(BUFFER_SIZE, OVERSAMPLE_FACTOR)?;
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let (mut engine, handle) = Engine::new(tuner, samplers, None, peak_meter)?;

    let input = vec![1.0f32; BUFFER_SIZE];
    let mut output = vec![0.0f32; BUFFER_SIZE];

    engine.process(&input, &mut output)?;
    let baseline_avg = output.iter().sum::<f32>() / output.len() as f32;

    let mut chain = AmplifierChain::new();
    chain.add_stage(Box::new(LevelStage::new(0.5)));
    handle.set_amp_chain(chain);

    output.fill(0.0);
    engine.process(&input, &mut output)?;
    let chain_avg = output.iter().sum::<f32>() / output.len() as f32;

    let ratio = chain_avg / baseline_avg;
    assert!(
        ratio < 0.6 && ratio > 0.4,
        "expected ~0.5x ratio, got {}",
        ratio
    );

    Ok(())
}

#[test]
fn samplers_preserve_tone_signal() -> Result<()> {
    const SAMPLE_RATE: usize = 48000;
    const BUFFER_SIZE: usize = 512;
    const TEST_FREQ: f32 = 440.0;

    for &oversample in &[1.0, 2.0, 4.0, 8.0, 16.0] {
        let mut samplers = Samplers::new(BUFFER_SIZE, oversample)?;

        let input: Vec<f32> = (0..BUFFER_SIZE)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                (2.0 * std::f32::consts::PI * TEST_FREQ * t).sin() * 0.5
            })
            .collect();

        samplers.copy_input(&input)?;

        let upsampled = samplers.upsample()?;
        let upsampled_rms =
            (upsampled.iter().map(|x| x * x).sum::<f32>() / upsampled.len() as f32).sqrt();

        let downsampled = samplers.downsample()?;
        let downsampled_rms =
            (downsampled.iter().map(|x| x * x).sum::<f32>() / downsampled.len() as f32).sqrt();

        let input_rms = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();

        println!(
            "{}x oversample: input_rms={:.4}, upsampled_rms={:.4}, downsampled_rms={:.4}, preservation={:.4}",
            oversample,
            input_rms,
            upsampled_rms,
            downsampled_rms,
            downsampled_rms / input_rms
        );

        let preservation_ratio = downsampled_rms / input_rms;
        assert!(
            preservation_ratio > 0.8 && preservation_ratio < 1.2,
            "{}x oversample: signal not preserved, got ratio {:.4}",
            oversample,
            preservation_ratio
        );
    }

    Ok(())
}

#[test]
fn engine_tuner_enabled_no_output() -> Result<()> {
    const SAMPLE_RATE: usize = 48000;
    const BUFFER_SIZE: usize = 128;
    const OVERSAMPLE_FACTOR: f64 = 1.0;
    let (mut tuner, _) = Tuner::new(SAMPLE_RATE);
    tuner.set_enabled(true);
    let samplers = Samplers::new(BUFFER_SIZE, OVERSAMPLE_FACTOR)?;
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let (mut engine, _) = Engine::new(tuner, samplers, None, peak_meter)?;

    let input = vec![0.0f32; BUFFER_SIZE];
    let mut output = vec![0.0f32; BUFFER_SIZE];
    engine.process(&input, &mut output)?;

    assert!(output.iter().all(|&x| x == 0.0), "expected silent output");

    Ok(())
}
