use arc_swap::ArcSwap;
use std::sync::Arc;

const CLIP_THRESHOLD: f32 = 0.95;

pub struct PeakMeter {
    current_peak: f32,
    samples_since_peak: usize,
    peak_hold_samples: usize,
    info: Arc<ArcSwap<PeakMeterInfo>>,
}

pub struct PeakMeterHandle {
    info: Arc<ArcSwap<PeakMeterInfo>>,
}

#[derive(Debug, Clone, Default)]
pub struct PeakMeterInfo {
    pub peak_db: f32,
    pub peak_linear: f32,
    pub is_clipping: bool,
}

impl PeakMeter {
    pub fn new(sample_rate: usize) -> (Self, PeakMeterHandle) {
        let info = Arc::new(ArcSwap::from_pointee(PeakMeterInfo::default()));

        (
            Self {
                current_peak: 0.0,
                samples_since_peak: 0,
                peak_hold_samples: sample_rate * 2, // 2 Seconds
                info: Arc::clone(&info),
            },
            PeakMeterHandle { info },
        )
    }

    pub fn process(&mut self, samples: &[f32]) {
        let block_peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

        if block_peak > self.current_peak {
            self.current_peak = block_peak;
            self.samples_since_peak = 0;
        } else {
            self.samples_since_peak += samples.len();

            if self.samples_since_peak > self.peak_hold_samples {
                self.current_peak = block_peak;
                self.samples_since_peak = 0;
            }
        }

        let peak_db = if self.current_peak > 1e-10 {
            20.0 * self.current_peak.log10()
        } else {
            -100.0
        };

        let is_clipping = self.current_peak >= CLIP_THRESHOLD;

        self.info.store(Arc::new(PeakMeterInfo {
            peak_db,
            peak_linear: self.current_peak,
            is_clipping,
        }));
    }

    pub fn reset(&mut self) {
        self.current_peak = 0.0;
        self.samples_since_peak = 0;
        self.info.store(Arc::new(PeakMeterInfo::default()));
    }
}

impl PeakMeterHandle {
    pub fn get_info(&self) -> PeakMeterInfo {
        self.info.load().as_ref().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_SAMPLE_RATE: usize = 48_000;

    #[test]
    fn test_peak_meter_detects_peaks() {
        let (mut meter, handle) = PeakMeter::new(TEST_SAMPLE_RATE);

        let silence = vec![0.0f32; 128];
        meter.process(&silence);

        let info = handle.get_info();
        assert!(info.peak_linear < 0.01);
        assert!(!info.is_clipping);

        let loud = vec![0.8f32; 128];
        meter.process(&loud);

        let info = handle.get_info();
        assert!((info.peak_linear - 0.8).abs() < 0.01);
        assert!(!info.is_clipping);

        let clipping = vec![0.99f32; 128];
        meter.process(&clipping);

        let info = handle.get_info();
        assert!(info.is_clipping);
        assert!(info.peak_linear > 0.95);
    }

    #[test]
    fn test_peak_meter_holds_peak() {
        let (mut meter, handle) = PeakMeter::new(TEST_SAMPLE_RATE);

        let loud = vec![0.8f32; 128];
        meter.process(&loud);

        let quiet = vec![0.2f32; 128];
        meter.process(&quiet);

        let info = handle.get_info();
        assert!(info.peak_linear > 0.7);
    }
}
