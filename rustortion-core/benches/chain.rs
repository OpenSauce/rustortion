#![allow(clippy::pedantic, clippy::nursery)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rustortion_core::amp::chain::AmplifierChain;
use rustortion_core::amp::stages::{
    clipper::ClipperType,
    compressor::CompressorStage,
    filter::{FilterStage, FilterType},
    level::LevelStage,
    nam::NamConfig,
    noise_gate::NoiseGateStage,
    poweramp::{PowerAmpStage, PowerAmpType},
    preamp::PreampStage,
    tonestack::{ToneStackModel, ToneStackStage},
};
use rustortion_core::nam::{NamLoader, registry};
use std::hint::black_box;
use std::path::Path;

const SAMPLE_RATE: usize = 48000;
const BUFFER_SIZE: usize = 128;

fn build_chain(sample_rate: f32) -> AmplifierChain {
    let mut chain = AmplifierChain::new();
    chain.add_stage(Box::new(FilterStage::new(
        FilterType::Highpass,
        100.0,
        sample_rate,
    )));
    chain.add_stage(Box::new(FilterStage::new(
        FilterType::Lowpass,
        8000.0,
        sample_rate,
    )));
    chain.add_stage(Box::new(NoiseGateStage::new(
        -40.0,
        10.0,
        1.0,
        10.0,
        100.0,
        sample_rate,
    )));
    chain.add_stage(Box::new(CompressorStage::new(
        2.0,
        100.0,
        -15.0,
        3.0,
        3.0,
        sample_rate,
    )));
    chain.add_stage(Box::new(PreampStage::new(
        6.0,
        0.0,
        ClipperType::Soft,
        sample_rate,
    )));
    chain.add_stage(Box::new(ToneStackStage::new(
        ToneStackModel::British,
        0.6,
        0.7,
        0.6,
        0.5,
        sample_rate,
    )));
    chain.add_stage(Box::new(PowerAmpStage::new(
        0.5,
        PowerAmpType::ClassAB,
        0.3,
        120.0,
        sample_rate,
    )));
    chain.add_stage(Box::new(LevelStage::new(0.8)));
    chain
}

fn bench_sample_vs_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("Sample vs Block Processing");

    for &oversample in &[1.0, 4.0, 8.0, 16.0] {
        let effective_sample_rate = (SAMPLE_RATE as f64 * oversample) as f32;
        let buffer_size = (BUFFER_SIZE as f64 * oversample) as usize;

        group.bench_with_input(
            BenchmarkId::new("sample-by-sample", format!("{oversample}x")),
            &oversample,
            |b, _| {
                let mut chain = build_chain(effective_sample_rate);
                let input: Vec<f32> = vec![0.5f32; buffer_size];

                b.iter(|| {
                    for &sample in &input {
                        black_box(chain.process(black_box(sample)));
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("block", format!("{oversample}x")),
            &oversample,
            |b, _| {
                let mut chain = build_chain(effective_sample_rate);
                let mut buffer: Vec<f32> = vec![0.5f32; buffer_size];

                b.iter(|| {
                    chain.process_block(black_box(&mut buffer));
                    black_box(&buffer);
                });
            },
        );
    }

    group.finish();
}

/// Load the vendored MIT reference WaveNet model (`tests/fixtures/`) into the global
/// registry and return its name. The fixture is committed, so the NAM benches run
/// deterministically in CI rather than depending on a user's gitignored `nam/` models.
fn load_first_nam_model() -> Option<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let loader = NamLoader::new(&dir).ok()?;
    registry::init_from_loader(&loader);
    // The fixture is a 48 kHz model and the chain runs at SAMPLE_RATE (48 kHz, 1x),
    // so the stage stays active rather than bypassing on a rate mismatch.
    registry::available_names().into_iter().next()
}

fn bench_nam_sample_vs_block(c: &mut Criterion) {
    let Some(model_name) = load_first_nam_model() else {
        eprintln!("skipping NAM bench: no .nam model found in tests/fixtures");
        return;
    };

    let config = NamConfig {
        model_name: Some(model_name),
        ..NamConfig::default()
    };

    // Sanity-check the model actually loaded (rate matches 48 kHz); if it bypassed we
    // would be benchmarking a passthrough, which is meaningless here.
    if !config.to_stage(SAMPLE_RATE as f32).is_active() {
        eprintln!("skipping NAM bench: model bypassed (sample-rate mismatch at 48 kHz)");
        return;
    }

    let mut group = c.benchmark_group("NAM Chain Sample vs Block");
    // NAM runs at the model's native rate (no oversampling), so benchmark at 1x only.
    let buffer_size = BUFFER_SIZE;

    group.bench_function(BenchmarkId::new("sample-by-sample", "1x"), |b| {
        let mut chain = build_chain(SAMPLE_RATE as f32);
        chain.add_stage(Box::new(config.to_stage(SAMPLE_RATE as f32)));
        let input: Vec<f32> = vec![0.5f32; buffer_size];

        b.iter(|| {
            for &sample in &input {
                black_box(chain.process(black_box(sample)));
            }
        });
    });

    group.bench_function(BenchmarkId::new("block", "1x"), |b| {
        let mut chain = build_chain(SAMPLE_RATE as f32);
        chain.add_stage(Box::new(config.to_stage(SAMPLE_RATE as f32)));
        let mut buffer: Vec<f32> = vec![0.5f32; buffer_size];

        b.iter(|| {
            chain.process_block(black_box(&mut buffer));
            black_box(&buffer);
        });
    });

    group.finish();
}

/// Isolated ceiling: raw nam-rs `process_buffer` (batched) vs a `process_sample`
/// loop on the same model, no chain, no gain/mix. This is the maximum speedup a
/// `NamStage::process_block` override could capture by calling `process_buffer`.
fn bench_nam_buffer_vs_sample(c: &mut Criterion) {
    let Some(model_name) = load_first_nam_model() else {
        eprintln!("skipping NAM ceiling bench: no .nam model found");
        return;
    };
    let Some(parsed) = registry::get(&model_name) else {
        return;
    };
    let Ok(mut model) = nam_rs::Model::from_nam(&parsed) else {
        eprintln!("skipping NAM ceiling bench: model failed to build");
        return;
    };

    let mut group = c.benchmark_group("NAM Model Buffer vs Sample");

    group.bench_function(BenchmarkId::new("process_sample-loop", "1x"), |b| {
        let input: Vec<f32> = vec![0.5f32; BUFFER_SIZE];
        b.iter(|| {
            for &sample in &input {
                black_box(model.process_sample(black_box(sample)));
            }
        });
    });

    group.bench_function(BenchmarkId::new("process_buffer", "1x"), |b| {
        let mut buffer: Vec<f32> = vec![0.5f32; BUFFER_SIZE];
        b.iter(|| {
            model.process_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sample_vs_block,
    bench_nam_sample_vs_block,
    bench_nam_buffer_vs_sample
);
criterion_main!(benches);
