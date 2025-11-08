use arc_swap::ArcSwap;
use std::sync::Arc;

const BUFFER_SIZE: usize = 4096;
const A1_HZ: f32 = 50.1;
const E6_HZ: f32 = 1245.0;

pub struct Tuner {
    buffer: Vec<f32>,
    sample_rate: usize,
    info: Arc<ArcSwap<TunerInfo>>,
    enabled: bool,
}

pub struct TunerHandle {
    info: Arc<ArcSwap<TunerInfo>>,
}

#[derive(Debug, Clone, Default)]
pub struct TunerInfo {
    pub frequency: Option<f32>,
    pub note: Option<String>,
    pub cents_off: Option<f32>,
    pub in_tune: bool,
}

impl Tuner {
    pub fn new(sample_rate: usize) -> (Self, TunerHandle) {
        let info = Arc::new(ArcSwap::from_pointee(TunerInfo::default()));

        (
            Self {
                buffer: Vec::with_capacity(BUFFER_SIZE),
                sample_rate,
                info: Arc::clone(&info),
                enabled: false,
            },
            TunerHandle { info },
        )
    }

    pub fn process(&mut self, samples: &[f32]) {
        if !self.enabled {
            return;
        }

        self.buffer.extend_from_slice(samples);
        if self.buffer.len() >= BUFFER_SIZE {
            let detected_frequency = self.simple_amdf();
            self.info.store(Arc::new(detected_frequency.into()));

            self.buffer.clear();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.buffer.clear();
            self.info.store(Arc::new(TunerInfo::default()));
        }
    }

    fn simple_amdf(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return None;
        }
        let rms =
            (self.buffer.iter().map(|x| x * x).sum::<f32>() / self.buffer.len() as f32).sqrt();

        if rms < 0.01 {
            return None;
        }

        let min_period = (self.sample_rate as f32 / E6_HZ) as usize;
        let max_period = (self.sample_rate as f32 / A1_HZ) as usize;

        let mut best_period = 0;
        let mut min_diff = f32::MAX;

        for lag in min_period..max_period.min(self.buffer.len() / 2) {
            let mut diff = 0.0;
            for i in 0..(self.buffer.len() - lag) {
                diff += (self.buffer[i] - self.buffer[i + lag]).abs();
            }

            if diff < min_diff {
                min_diff = diff;
                best_period = lag;
            }
        }

        if best_period > 0 {
            Some(self.sample_rate as f32 / best_period as f32)
        } else {
            None
        }
    }
}

impl TunerHandle {
    pub fn get_tuner_info(&self) -> TunerInfo {
        self.info.load().as_ref().clone()
    }
}

impl From<Option<f32>> for TunerInfo {
    fn from(freq: Option<f32>) -> Self {
        match freq {
            None => Self::default(),
            Some(f) => {
                let (note, octave, cents) = freq_to_note(f);
                Self {
                    frequency: Some(f),
                    note: Some(format!("{}{}", note, octave)),
                    cents_off: Some(cents),
                    in_tune: cents.abs() < 5.0,
                }
            }
        }
    }
}

fn freq_to_note(freq: f32) -> (&'static str, i32, f32) {
    let a4 = 440.0;

    let semitones_from_a4 = 12.0 * (freq / a4).log2();
    let note_number = semitones_from_a4.round() as i32;
    let cents = (semitones_from_a4 - note_number as f32) * 100.0;

    const NOTES: [&str; 12] = [
        "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
    ];
    let note_index = note_number.rem_euclid(12) as usize;
    let octave = 4 + (note_number + 9) / 12; // +9 because A is 9 semitones before C

    (NOTES[note_index], octave, cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_to_note() {
        let (note, octave, cents) = freq_to_note(440.0);
        assert_eq!(note, "A");
        assert_eq!(octave, 4);
        assert!(cents.abs() < 0.1);
    }
}
