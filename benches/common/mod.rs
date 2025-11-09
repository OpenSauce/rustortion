use hound::{WavSpec, WavWriter};
use rustortion::ir::cabinet::IrCabinet;
use std::fs;
use std::path::Path;

pub fn create_test_cabinet(ir_length: usize, sample_rate: usize) -> IrCabinet {
    let ir_dir = std::env::temp_dir().join("rustortion_bench_ir");
    fs::create_dir_all(&ir_dir).unwrap();

    let ir_path = ir_dir.join(format!("test_ir_{}.wav", ir_length));
    if !ir_path.exists() {
        create_synthetic_ir(&ir_path, ir_length, sample_rate as u32);
    }

    let mut cabinet = IrCabinet::new(&ir_dir, sample_rate).unwrap();
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
