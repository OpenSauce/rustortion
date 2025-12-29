use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rustortion::sim::chain::AmplifierChain;
use rustortion::sim::stages::{
    clipper::ClipperType,
    compressor::CompressorStage,
    filter::{FilterStage, FilterType},
    level::LevelStage,
    noise_gate::NoiseGateStage,
    poweramp::{PowerAmpStage, PowerAmpType},
    preamp::PreampStage,
    tonestack::{ToneStackModel, ToneStackStage},
};
use std::hint::black_box;

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
    chain.add_stage(Box::new(PreampStage::new(6.0, 0.0, ClipperType::Soft)));
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
            BenchmarkId::new("sample-by-sample", format!("{}x", oversample)),
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
            BenchmarkId::new("block", format!("{}x", oversample)),
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

criterion_group!(benches, bench_sample_vs_block);
criterion_main!(benches);
