use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use hound::{WavSpec, WavWriter};
use rustortion::ir::cabinet::IrCabinet;
use std::fs;
use std::hint::black_box;
use std::path::Path;

const SAMPLE_RATE: u32 = 48000;
const FFT_BLOCK_SIZE: usize = 1024;

pub fn impulse_response_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Impulse Responses");

    for &len in &[1_000, 13_000, 34_000, 87_000] {
        group.bench_with_input(BenchmarkId::from_parameter(len), &len, |b, &len| {
            let mut cabinet = create_test_cabinet(len);
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

fn create_test_cabinet(ir_length: usize) -> IrCabinet {
    let ir_dir = std::env::temp_dir().join("rustortion_bench_ir");
    fs::create_dir_all(&ir_dir).unwrap();

    let ir_path = ir_dir.join(format!("test_ir_{}.wav", ir_length));
    if !ir_path.exists() {
        create_synthetic_ir(&ir_path, ir_length, SAMPLE_RATE);
    }

    let mut cabinet = IrCabinet::new(&ir_dir, SAMPLE_RATE).unwrap();
    cabinet
        .select_ir(&format!("test_ir_{}.wav", ir_length))
        .unwrap();

    cabinet
}

fn create_synthetic_ir(path: &Path, length: usize, sample_rate: u32) {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec).unwrap();

    for i in 0..length {
        let t = i as f32 / sample_rate as f32;
        let decay = (-t * 3.0).exp();
        let freq = 440.0 * 2.0 * std::f32::consts::PI;
        let sample = (freq * t).sin() * decay;
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16).unwrap();
    }

    writer.finalize().unwrap();
}

criterion_group!(
    benches,
    impulse_response_benchmarks,
    convolution_loop_benchmark
);
criterion_main!(benches);
