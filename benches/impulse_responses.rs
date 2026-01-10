use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use rustortion::ir::convolver::{FirConvolver, TwoStageConvolver};

const SAMPLE_RATE: usize = 48000;
const BUFFER_SIZE: usize = 128;

/// Generate a synthetic IR with exponential decay (simulates cabinet response)
fn generate_test_ir(length: usize) -> Vec<f32> {
    (0..length)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let decay = (-t * 30.0).exp();
            let freq = 1000.0 * std::f32::consts::PI * 2.0;
            (freq * t).sin() * decay * 0.5
        })
        .collect()
}

/// Generate test input signal
fn generate_test_input(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            (440.0 * std::f32::consts::PI * 2.0 * t).sin() * 0.5
        })
        .collect()
}

pub fn fir_vs_two_stage_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("FIR vs TwoStage");

    // Test at different IR lengths relevant to cabinet simulation
    // 1000 = ~21ms, 2400 = 50ms, 4800 = 100ms
    for &ir_len in &[1_000, 2_400, 4_800] {
        let ir = generate_test_ir(ir_len);
        let input = generate_test_input(BUFFER_SIZE);
        let ir_ms = (ir_len as f32 / SAMPLE_RATE as f32) * 1000.0;

        // FIR Convolver
        group.bench_with_input(
            BenchmarkId::new("FIR", format!("{:.0}ms", ir_ms)),
            &ir_len,
            |b, _| {
                let mut conv = FirConvolver::new(ir_len);
                conv.set_ir(&ir).unwrap();

                // Warmup
                for _ in 0..100 {
                    let mut buf = input.clone();
                    conv.process_block(&mut buf);
                }

                b.iter(|| {
                    let mut buf = input.clone();
                    conv.process_block(black_box(&mut buf));
                    black_box(&buf);
                });
            },
        );

        // TwoStage Convolver
        group.bench_with_input(
            BenchmarkId::new("TwoStage", format!("{:.0}ms", ir_ms)),
            &ir_len,
            |b, _| {
                let mut conv = TwoStageConvolver::new();
                conv.set_ir(&ir).unwrap();

                // Warmup
                for _ in 0..100 {
                    let mut buf = input.clone();
                    conv.process_block(&mut buf);
                }

                b.iter(|| {
                    let mut buf = input.clone();
                    conv.process_block(black_box(&mut buf));
                    black_box(&buf);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, fir_vs_two_stage_benchmark,);
criterion_main!(benches);
