use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use rubato::{
    FastFixedIn, FftFixedIn, FftFixedOut, PolynomialDegree, Resampler, SincFixedIn,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

const SAMPLE_RATE: usize = 48000;
const BUFFER_SIZE: usize = 128;
const CHANNELS: usize = 1;
const SUB_CHUNKS: usize = 2;

fn generate_test_signal(size: usize) -> Vec<Vec<f32>> {
    let signal: Vec<f32> = (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();
    vec![signal]
}

// ============================================================================
// Resampler Pairs
// ============================================================================

struct SincPair {
    up: SincFixedIn<f32>,
    down: SincFixedIn<f32>,
}

struct FastPair {
    up: FastFixedIn<f32>,
    down: FastFixedIn<f32>,
}

struct FftPair {
    up: FftFixedIn<f32>,
    down: FftFixedOut<f32>,
}

fn create_sinc_current(factor: usize) -> SincPair {
    let up_params = SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };
    let down_params = SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };

    SincPair {
        up: SincFixedIn::new(factor as f64, 1.0, up_params, BUFFER_SIZE, CHANNELS).unwrap(),
        down: SincFixedIn::new(
            1.0 / factor as f64,
            1.0,
            down_params,
            BUFFER_SIZE * factor,
            CHANNELS,
        )
        .unwrap(),
    }
}

fn create_sinc_veryfast(factor: usize) -> SincPair {
    let up_params = SincInterpolationParameters {
        sinc_len: 64,
        f_cutoff: 0.91,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 1024,
        window: WindowFunction::Hann2,
    };
    let down_params = SincInterpolationParameters {
        sinc_len: 64,
        f_cutoff: 0.91,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 1024,
        window: WindowFunction::Hann2,
    };

    SincPair {
        up: SincFixedIn::new(factor as f64, 1.0, up_params, BUFFER_SIZE, CHANNELS).unwrap(),
        down: SincFixedIn::new(
            1.0 / factor as f64,
            1.0,
            down_params,
            BUFFER_SIZE * factor,
            CHANNELS,
        )
        .unwrap(),
    }
}

fn create_fast_cubic(factor: usize) -> FastPair {
    FastPair {
        up: FastFixedIn::new(
            factor as f64,
            1.0,
            PolynomialDegree::Cubic,
            BUFFER_SIZE,
            CHANNELS,
        )
        .unwrap(),
        down: FastFixedIn::new(
            1.0 / factor as f64,
            1.0,
            PolynomialDegree::Cubic,
            BUFFER_SIZE * factor,
            CHANNELS,
        )
        .unwrap(),
    }
}

fn create_fast_linear(factor: usize) -> FastPair {
    FastPair {
        up: FastFixedIn::new(
            factor as f64,
            1.0,
            PolynomialDegree::Linear,
            BUFFER_SIZE,
            CHANNELS,
        )
        .unwrap(),
        down: FastFixedIn::new(
            1.0 / factor as f64,
            1.0,
            PolynomialDegree::Linear,
            BUFFER_SIZE * factor,
            CHANNELS,
        )
        .unwrap(),
    }
}

fn create_fft(factor: usize) -> FftPair {
    FftPair {
        up: FftFixedIn::new(
            SAMPLE_RATE,
            SAMPLE_RATE * factor,
            BUFFER_SIZE,
            SUB_CHUNKS,
            CHANNELS,
        )
        .unwrap(),
        down: FftFixedOut::new(
            SAMPLE_RATE * factor,
            SAMPLE_RATE,
            BUFFER_SIZE,
            SUB_CHUNKS,
            CHANNELS,
        )
        .unwrap(),
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_resampler_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("Resampler Roundtrip");
    group.throughput(Throughput::Elements(BUFFER_SIZE as u64));

    for &factor in &[2usize, 4, 8] {
        let input = generate_test_signal(BUFFER_SIZE);

        group.bench_with_input(
            BenchmarkId::new("SincFixedIn-Current", format!("{}x", factor)),
            &factor,
            |b, &factor| {
                let mut pair = create_sinc_current(factor);
                let mut up_buf = pair.up.output_buffer_allocate(true);
                let mut down_buf = pair.down.output_buffer_allocate(true);

                b.iter(|| {
                    pair.up
                        .process_into_buffer(black_box(&input), &mut up_buf, None)
                        .unwrap();
                    pair.down
                        .process_into_buffer(black_box(&up_buf), &mut down_buf, None)
                        .unwrap();
                    black_box(&down_buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("SincFixedIn-VeryFast", format!("{}x", factor)),
            &factor,
            |b, &factor| {
                let mut pair = create_sinc_veryfast(factor);
                let mut up_buf = pair.up.output_buffer_allocate(true);
                let mut down_buf = pair.down.output_buffer_allocate(true);

                b.iter(|| {
                    pair.up
                        .process_into_buffer(black_box(&input), &mut up_buf, None)
                        .unwrap();
                    pair.down
                        .process_into_buffer(black_box(&up_buf), &mut down_buf, None)
                        .unwrap();
                    black_box(&down_buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("FastFixedIn-Cubic", format!("{}x", factor)),
            &factor,
            |b, &factor| {
                let mut pair = create_fast_cubic(factor);
                let mut up_buf = pair.up.output_buffer_allocate(true);
                let mut down_buf = pair.down.output_buffer_allocate(true);

                b.iter(|| {
                    pair.up
                        .process_into_buffer(black_box(&input), &mut up_buf, None)
                        .unwrap();
                    pair.down
                        .process_into_buffer(black_box(&up_buf), &mut down_buf, None)
                        .unwrap();
                    black_box(&down_buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("FastFixedIn-Linear", format!("{}x", factor)),
            &factor,
            |b, &factor| {
                let mut pair = create_fast_linear(factor);
                let mut up_buf = pair.up.output_buffer_allocate(true);
                let mut down_buf = pair.down.output_buffer_allocate(true);

                b.iter(|| {
                    pair.up
                        .process_into_buffer(black_box(&input), &mut up_buf, None)
                        .unwrap();
                    pair.down
                        .process_into_buffer(black_box(&up_buf), &mut down_buf, None)
                        .unwrap();
                    black_box(&down_buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("FftFixed", format!("{}x", factor)),
            &factor,
            |b, &factor| {
                let mut pair = create_fft(factor);
                let mut up_buf = pair.up.output_buffer_allocate(true);
                let mut down_buf = pair.down.output_buffer_allocate(true);

                b.iter(|| {
                    pair.up
                        .process_into_buffer(black_box(&input), &mut up_buf, None)
                        .unwrap();
                    pair.down
                        .process_into_buffer(black_box(&up_buf), &mut down_buf, None)
                        .unwrap();
                    black_box(&down_buf);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_resampler_roundtrip);
criterion_main!(benches);
