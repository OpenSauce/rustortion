use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

mod common;
use common::create_test_cabinet;

const SAMPLE_RATE: usize = 48000;
const FFT_BLOCK_SIZE: usize = 1024;

pub fn impulse_response_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Impulse Responses");

    for &len in &[1_000, 13_000, 34_000, 87_000] {
        group.bench_with_input(BenchmarkId::from_parameter(len), &len, |b, &len| {
            let mut cabinet = create_test_cabinet(len, SAMPLE_RATE);
            let mut samples = vec![0.5f32; 128];

            for _ in 0..100 {
                cabinet.process_block(&mut samples);
            }

            b.iter(|| {
                cabinet.process_block(black_box(&mut samples));
            });
        });
    }

    group.finish();
}

pub fn convolution_loop_benchmark(c: &mut Criterion) {
    use rustfft::num_complex::Complex;

    let num_bins = FFT_BLOCK_SIZE / 2 + 1;
    let num_partitions = 34;

    let history: Vec<Vec<Complex<f32>>> =
        vec![vec![Complex::new(0.5, 0.3); num_bins]; num_partitions];
    let ir_partitions: Vec<Vec<Complex<f32>>> =
        vec![vec![Complex::new(0.7, 0.2); num_bins]; num_partitions];

    c.bench_function("Convolution Loop", |b| {
        let mut accumulator = vec![Complex::new(0.0, 0.0); num_bins];
        b.iter(|| {
            accumulator.fill(Complex::new(0.0, 0.0));
            for j in 0..num_partitions {
                for (k, acc) in accumulator.iter_mut().enumerate().take(num_bins) {
                    *acc += black_box(history[j][k]) * black_box(ir_partitions[j][k]);
                }
            }
            black_box(&accumulator);
        });
    });
}

criterion_group!(
    benches,
    impulse_response_benchmarks,
    convolution_loop_benchmark
);
criterion_main!(benches);
