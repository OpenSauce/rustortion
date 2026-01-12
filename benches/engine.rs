use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use rustortion::amp::chain::AmplifierChain;
use rustortion::amp::stages::level::LevelStage;
use rustortion::audio::engine::{Engine, EngineHandle};
use rustortion::audio::peak_meter::PeakMeter;
use rustortion::audio::samplers::Samplers;
use rustortion::metronome::Metronome;
use rustortion::tuner::Tuner;

mod common;
use common::create_test_cabinet;

const SAMPLE_RATE: usize = 48000;
const BUFFER_SIZE: usize = 128;
const OVERSAMPLE: f64 = 4.0;

pub fn build_engine(
    oversample: f64,
    buffer_size: usize,
    ir_length: Option<usize>,
) -> (Engine, EngineHandle) {
    let ir_cabinet = ir_length.map(|len| create_test_cabinet(len, SAMPLE_RATE));
    let (tuner, _) = Tuner::new(SAMPLE_RATE);
    let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
    let samplers = Samplers::new(buffer_size, oversample, SAMPLE_RATE).unwrap();
    let metronome = Metronome::new(120.0, SAMPLE_RATE);
    let (engine, handle) = Engine::new(tuner, samplers, ir_cabinet, peak_meter, metronome).unwrap();
    (engine, handle)
}

fn bench_engine_buffer_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("Engine Buffer Sizes");

    for &buffer_size in &[64, 128, 256, 512] {
        group.bench_with_input(
            BenchmarkId::from_parameter(buffer_size),
            &buffer_size,
            |b, &buffer_size| {
                let (mut engine, handle) = build_engine(OVERSAMPLE, buffer_size, None);

                let mut chain = AmplifierChain::new();
                chain.add_stage(Box::new(LevelStage::new(0.5)));
                handle.set_amp_chain(chain);

                let input = vec![0.5f32; buffer_size];
                let mut output = vec![0.0f32; buffer_size];

                engine.process(&input, &mut output).unwrap();

                b.iter(|| {
                    engine
                        .process(black_box(&input), black_box(&mut output))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_engine_throughput(c: &mut Criterion) {
    use criterion::Throughput;

    let mut group = c.benchmark_group("Engine Throughput");

    for &oversample in &[1.0, 4.0, 8.0] {
        group.throughput(Throughput::Elements(BUFFER_SIZE as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x", oversample)),
            &oversample,
            |b, &oversample| {
                let (mut engine, _) = build_engine(oversample, BUFFER_SIZE, None);

                let input = vec![0.5f32; BUFFER_SIZE];
                let mut output = vec![0.0f32; BUFFER_SIZE];

                b.iter(|| {
                    engine
                        .process(black_box(&input), black_box(&mut output))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_engine_with_ir_cabinet(c: &mut Criterion) {
    let mut group = c.benchmark_group("Engine With IR Cabinet");

    for &oversample in &[1.0, 4.0, 8.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x", oversample)),
            &oversample,
            |b, &oversample| {
                let (tuner, _) = Tuner::new(SAMPLE_RATE);
                let samplers = Samplers::new(BUFFER_SIZE, oversample, SAMPLE_RATE).unwrap();
                let ir_cabinet = Some(create_test_cabinet(20000, SAMPLE_RATE));
                let (peak_meter, _) = PeakMeter::new(SAMPLE_RATE);
                let metronome = Metronome::new(120.0, SAMPLE_RATE);
                let (mut engine, _) =
                    Engine::new(tuner, samplers, ir_cabinet, peak_meter, metronome).unwrap();

                let input = vec![0.5f32; BUFFER_SIZE];
                let mut output = vec![0.0f32; BUFFER_SIZE];

                b.iter(|| {
                    engine
                        .process(black_box(&input), black_box(&mut output))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_engine_ir_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("Engine IR Lengths");

    for &ir_length in &[1_000, 13_000, 34_000, 87_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{} samples", ir_length)),
            &ir_length,
            |b, &ir_length| {
                let (mut engine, _) = build_engine(OVERSAMPLE, BUFFER_SIZE, Some(ir_length));

                let input = vec![0.5f32; BUFFER_SIZE];
                let mut output = vec![0.0f32; BUFFER_SIZE];

                b.iter(|| {
                    engine
                        .process(black_box(&input), black_box(&mut output))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_engine_buffer_sizes,
    bench_engine_throughput,
    bench_engine_with_ir_cabinet,
    bench_engine_ir_lengths,
);
criterion_main!(benches);
