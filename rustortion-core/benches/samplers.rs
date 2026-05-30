#![allow(clippy::pedantic, clippy::nursery)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, Fft, FixedAsync, FixedSync, PolynomialDegree, Resampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
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

struct ResamplerPair {
    up: Box<dyn Resampler<f32>>,
    down: Box<dyn Resampler<f32>>,
}

type PairFactory = fn(usize) -> ResamplerPair;

fn create_sinc_current(factor: usize) -> ResamplerPair {
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

    ResamplerPair {
        up: Box::new(
            Async::<f32>::new_sinc(
                factor as f64,
                1.0,
                &up_params,
                BUFFER_SIZE,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
        down: Box::new(
            Async::<f32>::new_sinc(
                1.0 / factor as f64,
                1.0,
                &down_params,
                BUFFER_SIZE * factor,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
    }
}

fn create_sinc_veryfast(factor: usize) -> ResamplerPair {
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

    ResamplerPair {
        up: Box::new(
            Async::<f32>::new_sinc(
                factor as f64,
                1.0,
                &up_params,
                BUFFER_SIZE,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
        down: Box::new(
            Async::<f32>::new_sinc(
                1.0 / factor as f64,
                1.0,
                &down_params,
                BUFFER_SIZE * factor,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
    }
}

fn create_fast_cubic(factor: usize) -> ResamplerPair {
    ResamplerPair {
        up: Box::new(
            Async::<f32>::new_poly(
                factor as f64,
                1.0,
                PolynomialDegree::Cubic,
                BUFFER_SIZE,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
        down: Box::new(
            Async::<f32>::new_poly(
                1.0 / factor as f64,
                1.0,
                PolynomialDegree::Cubic,
                BUFFER_SIZE * factor,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
    }
}

fn create_fast_linear(factor: usize) -> ResamplerPair {
    ResamplerPair {
        up: Box::new(
            Async::<f32>::new_poly(
                factor as f64,
                1.0,
                PolynomialDegree::Linear,
                BUFFER_SIZE,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
        down: Box::new(
            Async::<f32>::new_poly(
                1.0 / factor as f64,
                1.0,
                PolynomialDegree::Linear,
                BUFFER_SIZE * factor,
                CHANNELS,
                FixedAsync::Input,
            )
            .unwrap(),
        ),
    }
}

fn create_fft(factor: usize) -> ResamplerPair {
    ResamplerPair {
        up: Box::new(
            Fft::<f32>::new(
                SAMPLE_RATE,
                SAMPLE_RATE * factor,
                BUFFER_SIZE,
                SUB_CHUNKS,
                CHANNELS,
                FixedSync::Input,
            )
            .unwrap(),
        ),
        down: Box::new(
            Fft::<f32>::new(
                SAMPLE_RATE * factor,
                SAMPLE_RATE,
                BUFFER_SIZE,
                SUB_CHUNKS,
                CHANNELS,
                FixedSync::Output,
            )
            .unwrap(),
        ),
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn run_roundtrip(pair: &mut ResamplerPair, input: &[Vec<f32>]) {
    let in_frames = input[0].len();
    let up_cap = pair.up.output_frames_max();
    let down_cap = pair.down.output_frames_max();

    let mut up_buf = vec![vec![0.0f32; up_cap]; CHANNELS];
    let mut down_buf = vec![vec![0.0f32; down_cap]; CHANNELS];

    let in_adapter = SequentialSliceOfVecs::new(black_box(input), CHANNELS, in_frames).unwrap();
    let mut up_adapter = SequentialSliceOfVecs::new_mut(&mut up_buf, CHANNELS, up_cap).unwrap();
    pair.up
        .process_into_buffer(&in_adapter, &mut up_adapter, None)
        .unwrap();

    let up_in = SequentialSliceOfVecs::new(&up_buf, CHANNELS, up_cap).unwrap();
    let mut down_adapter =
        SequentialSliceOfVecs::new_mut(&mut down_buf, CHANNELS, down_cap).unwrap();
    pair.down
        .process_into_buffer(&up_in, &mut down_adapter, None)
        .unwrap();

    black_box(&down_buf);
}

fn bench_resampler_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("Resampler Roundtrip");
    group.throughput(Throughput::Elements(BUFFER_SIZE as u64));

    let variants: &[(&str, PairFactory)] = &[
        ("SincFixedIn-Current", create_sinc_current),
        ("SincFixedIn-VeryFast", create_sinc_veryfast),
        ("FastFixedIn-Cubic", create_fast_cubic),
        ("FastFixedIn-Linear", create_fast_linear),
        ("FftFixed", create_fft),
    ];

    for &factor in &[2usize, 4, 8] {
        let input = generate_test_signal(BUFFER_SIZE);

        for &(name, create) in variants {
            group.bench_with_input(
                BenchmarkId::new(name, format!("{factor}x")),
                &factor,
                |b, &factor| {
                    let mut pair = create(factor);
                    b.iter(|| run_roundtrip(&mut pair, &input));
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_resampler_roundtrip);
criterion_main!(benches);
