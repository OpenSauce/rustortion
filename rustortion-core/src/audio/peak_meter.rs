use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const CLIP_THRESHOLD: f32 = 0.95;

/// Lock-free, allocation-free shared peak-meter readout.
///
/// `PeakMeterInfo` is plain data with no need for reference counting, so the
/// three fields are stored as atomics rather than swapped behind an `Arc`.
/// This keeps `PeakMeter::process` (called per audio block on the RT thread)
/// free of the `Arc::new` allocation the previous `ArcSwap` design incurred.
///
/// `f32` values are stored as their bit patterns. All access is `Relaxed`:
/// the fields are independent and a momentarily-torn read across them is
/// cosmetically irrelevant for a level meter.
struct PeakMeterShared {
    peak_db: AtomicU32,
    peak_linear: AtomicU32,
    is_clipping: AtomicBool,
}

impl PeakMeterShared {
    fn new() -> Self {
        let default = PeakMeterInfo::default();
        Self {
            peak_db: AtomicU32::new(default.peak_db.to_bits()),
            peak_linear: AtomicU32::new(default.peak_linear.to_bits()),
            is_clipping: AtomicBool::new(default.is_clipping),
        }
    }

    fn store(&self, peak_db: f32, peak_linear: f32, is_clipping: bool) {
        self.peak_db.store(peak_db.to_bits(), Ordering::Relaxed);
        self.peak_linear
            .store(peak_linear.to_bits(), Ordering::Relaxed);
        self.is_clipping.store(is_clipping, Ordering::Relaxed);
    }

    fn load(&self) -> PeakMeterInfo {
        PeakMeterInfo {
            peak_db: f32::from_bits(self.peak_db.load(Ordering::Relaxed)),
            peak_linear: f32::from_bits(self.peak_linear.load(Ordering::Relaxed)),
            is_clipping: self.is_clipping.load(Ordering::Relaxed),
        }
    }
}

pub struct PeakMeter {
    current_peak: f32,
    samples_since_peak: usize,
    peak_hold_samples: usize,
    shared: Arc<PeakMeterShared>,
}

pub struct PeakMeterHandle {
    shared: Arc<PeakMeterShared>,
}

#[derive(Debug, Clone, Default)]
pub struct PeakMeterInfo {
    pub peak_db: f32,
    pub peak_linear: f32,
    pub is_clipping: bool,
}

impl PeakMeter {
    pub fn new(sample_rate: usize) -> (Self, PeakMeterHandle) {
        let shared = Arc::new(PeakMeterShared::new());

        (
            Self {
                current_peak: 0.0,
                samples_since_peak: 0,
                peak_hold_samples: sample_rate * 2, // 2 Seconds
                shared: Arc::clone(&shared),
            },
            PeakMeterHandle { shared },
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

        self.shared.store(peak_db, self.current_peak, is_clipping);
    }

    pub fn reset(&mut self) {
        self.current_peak = 0.0;
        self.samples_since_peak = 0;
        let default = PeakMeterInfo::default();
        self.shared
            .store(default.peak_db, default.peak_linear, default.is_clipping);
    }
}

impl PeakMeterHandle {
    pub fn get_info(&self) -> PeakMeterInfo {
        self.shared.load()
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
