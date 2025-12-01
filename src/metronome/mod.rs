use core::f32;
use hound::WavReader;
use log::error;
use std::fs::File;
use std::io::BufReader;

pub struct Metronome {
    bpm: f32,
    sample_rate: usize,
    enabled: bool,
    tick_buffer: Vec<f32>,
    interval: usize,
    samples_processed: usize,
    buffer_index: usize,
}

impl Metronome {
    pub fn new(bpm: f32, sample_rate: usize) -> Self {
        Self {
            bpm,
            sample_rate,
            enabled: false,
            tick_buffer: Vec::new(),
            interval: (sample_rate as f32 / (bpm / 60.0)) as usize,
            samples_processed: 0,
            buffer_index: 0,
        }
    }

    pub fn load_wav_file(&mut self, file_path: &str) {
        let file = match File::open(file_path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to read file '{file_path}' : {e}");
                return;
            }
        };
        let mut reader = WavReader::new(BufReader::new(file)).unwrap();
        let spec = reader.spec();
        println!("{:?}", spec);
        if spec.sample_rate != self.sample_rate as u32 {
            //resample
            let samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect();
            self.tick_buffer =
                Self::resample_tick_file(&samples, spec.sample_rate, self.sample_rate as u32);
        } else {
            self.tick_buffer = reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect();
        }
    }

    pub fn is_enabled(&mut self) -> bool {
        return self.enabled;
    }

    pub fn process_block(&mut self, output: &mut [f32]) {
        //handle metronome logic
        for i in output {
            if self.samples_processed >= 0 && self.samples_processed < self.tick_buffer.len() {
                *i = self.tick_buffer[self.buffer_index];
                self.samples_processed = if self.samples_processed == self.interval {
                    0
                } else {
                    self.samples_processed + 1
                };
                self.buffer_index = if self.buffer_index < self.tick_buffer.len() {
                    self.buffer_index + 1
                } else {
                    0
                };
            }
        }
    }

    pub fn resample_tick_file(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (samples.len() as f64 / ratio) as usize;
        let mut out = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_pos = i as f64 * ratio;
            let src_idx = src_pos as usize;
            let frac = src_pos - src_idx as f64;

            let s = if src_idx + 1 < samples.len() {
                samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
            } else if src_idx < samples.len() {
                samples[src_idx]
            } else {
                0.0
            };
            out.push(s);
        }
        out
    }

    pub fn toggle_metronome(&mut self) {
        self.enabled = !self.enabled;
    }
}
