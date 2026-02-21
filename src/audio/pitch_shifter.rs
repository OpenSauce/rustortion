use std::f64::consts::PI;
use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

/// Interpolate between two phases using shortest-path unwrapping.
fn lerp_phase(ph0: f64, ph1: f64, t: f64) -> f64 {
    let mut d = ph1 - ph0;
    d -= (d / (2.0 * PI)).round() * 2.0 * PI;
    ph0 + d * t
}

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 8; // 87.5% overlap
const NUM_BINS: usize = FFT_SIZE / 2 + 1;
const OUTPUT_SIZE: usize = FFT_SIZE * 2;

/// Phase-vocoder pitch shifter with phase locking.
///
/// Analyzes overlapping STFT frames, shifts frequency bins by the pitch ratio,
/// and uses identity phase locking to maintain coherent phase relationships
/// within each spectral peak, eliminating the "phasiness" / doubled quality
/// of basic phase vocoders.
///
/// Adds ~`FFT_SIZE / sample_rate` latency (≈43 ms at 48 kHz).
pub struct PitchShifter {
    ratio: f64,

    // FFT plans
    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,

    // Input ring buffer (last FFT_SIZE samples)
    input_ring: Vec<f32>,
    input_pos: usize,
    hop_counter: usize,

    // Phase vocoder state per bin
    last_phase: Vec<f64>,
    accum_phase: Vec<f64>,

    // Output overlap-add buffer
    output_accum: Vec<f32>,
    output_read: usize,
    output_write: usize,

    // Hann window
    window: Vec<f32>,
    output_scale: f32,

    // Pre-allocated scratch buffers
    frame: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    shifted: Vec<Complex<f32>>,
    synth_frame: Vec<f32>,
    r2c_scratch: Vec<Complex<f32>>,
    c2r_scratch: Vec<Complex<f32>>,
    analysis_mag: Vec<f64>,
    analysis_freq: Vec<f64>,
    analysis_phase: Vec<f64>,
    shifted_mag: Vec<f64>,
    shifted_phase: Vec<f64>,
    peak_bin: Vec<usize>,
    peaks: Vec<usize>,
    first_frame: bool,
}

impl PitchShifter {
    pub fn new(semitones: f32) -> Self {
        let ratio = (2.0_f64).powf(semitones as f64 / 12.0);

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(FFT_SIZE);
        let c2r = planner.plan_fft_inverse(FFT_SIZE);
        let r2c_scratch = r2c.make_scratch_vec();
        let c2r_scratch = c2r.make_scratch_vec();

        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / FFT_SIZE as f64).cos()) as f32)
            .collect();

        // Compute the COLA (Constant Overlap-Add) normalization for Hann²
        // with the current hop size, rather than relying on a magic constant.
        // For hop = N/8 this evaluates to 3.0, but computing it makes the code
        // robust to future hop/window changes.
        let num_overlaps = FFT_SIZE / HOP_SIZE;
        let cola_sum: f32 = (0..HOP_SIZE)
            .map(|i| {
                (0..num_overlaps)
                    .map(|m| {
                        let idx = i + m * HOP_SIZE;
                        window[idx] * window[idx]
                    })
                    .sum::<f32>()
            })
            .sum::<f32>()
            / HOP_SIZE as f32;
        let output_scale = 1.0 / (FFT_SIZE as f32 * cola_sum);

        Self {
            ratio,
            r2c,
            c2r,
            input_ring: vec![0.0; FFT_SIZE],
            input_pos: 0,
            hop_counter: 0,
            last_phase: vec![0.0; NUM_BINS],
            accum_phase: vec![0.0; NUM_BINS],
            output_accum: vec![0.0; OUTPUT_SIZE],
            output_read: 0,
            output_write: HOP_SIZE,
            window,
            output_scale,
            frame: vec![0.0; FFT_SIZE],
            spectrum: vec![Complex::new(0.0, 0.0); NUM_BINS],
            shifted: vec![Complex::new(0.0, 0.0); NUM_BINS],
            synth_frame: vec![0.0; FFT_SIZE],
            r2c_scratch,
            c2r_scratch,
            analysis_mag: vec![0.0; NUM_BINS],
            analysis_freq: vec![0.0; NUM_BINS],
            analysis_phase: vec![0.0; NUM_BINS],
            shifted_mag: vec![0.0; NUM_BINS],
            shifted_phase: vec![0.0; NUM_BINS],
            peak_bin: vec![0; NUM_BINS],
            peaks: Vec::with_capacity(64),
            first_frame: true,
        }
    }

    /// Update the pitch ratio without reallocating buffers.
    pub fn set_semitones(&mut self, semitones: f32) {
        self.ratio = (2.0_f64).powf(semitones as f64 / 12.0);
        self.last_phase.fill(0.0);
        self.accum_phase.fill(0.0);
        self.first_frame = true;
    }

    pub fn process_block(&mut self, data: &mut [f32]) {
        for sample in data.iter_mut() {
            // Feed input into ring buffer
            self.input_ring[self.input_pos] = *sample;
            self.input_pos = (self.input_pos + 1) % FFT_SIZE;

            self.hop_counter += 1;
            if self.hop_counter >= HOP_SIZE {
                self.hop_counter = 0;
                self.process_frame();
            }

            // Read from overlap-add output
            *sample = self.output_accum[self.output_read];
            self.output_accum[self.output_read] = 0.0;
            self.output_read = (self.output_read + 1) % OUTPUT_SIZE;
        }
    }

    fn process_frame(&mut self) {
        let expected_step = 2.0 * PI * HOP_SIZE as f64 / FFT_SIZE as f64;

        // --- Analysis ---
        for i in 0..FFT_SIZE {
            let idx = (self.input_pos + i) % FFT_SIZE;
            self.frame[i] = self.input_ring[idx] * self.window[i];
        }

        self.r2c
            .process_with_scratch(&mut self.frame, &mut self.spectrum, &mut self.r2c_scratch)
            .expect("FFT failed");

        for k in 0..NUM_BINS {
            let re = self.spectrum[k].re as f64;
            let im = self.spectrum[k].im as f64;

            let mag = re.hypot(im);
            let phase = im.atan2(re);

            let mut diff = phase - self.last_phase[k] - k as f64 * expected_step;
            self.last_phase[k] = phase;

            // Wrap to [-π, π]
            diff -= (diff / (2.0 * PI)).round() * 2.0 * PI;

            self.analysis_mag[k] = mag;
            self.analysis_freq[k] = k as f64 * expected_step + diff;
            self.analysis_phase[k] = phase;
        }

        // --- Shift bins and compute raw accumulated phase ---
        self.shifted_mag.fill(0.0);
        self.shifted_phase.fill(0.0);

        for j in 0..NUM_BINS {
            let source = j as f64 / self.ratio;
            let src_k = source.floor() as usize;
            if src_k >= NUM_BINS - 1 {
                continue;
            }

            let frac = source - src_k as f64;

            let mag = self.analysis_mag[src_k]
                + (self.analysis_mag[src_k + 1] - self.analysis_mag[src_k]) * frac;
            let freq = (self.analysis_freq[src_k]
                + (self.analysis_freq[src_k + 1] - self.analysis_freq[src_k]) * frac)
                * self.ratio;

            if self.first_frame {
                // Seed from interpolated analysis phase to avoid startup smear
                let phase = lerp_phase(
                    self.analysis_phase[src_k],
                    self.analysis_phase[src_k + 1],
                    frac,
                );
                self.accum_phase[j] = phase;
            } else {
                self.accum_phase[j] += freq;
            }

            // Wrap to [0, 2π) to prevent precision loss over long sessions
            self.accum_phase[j] -= (self.accum_phase[j] / (2.0 * PI)).floor() * 2.0 * PI;

            self.shifted_mag[j] = mag;
            self.shifted_phase[j] = self.accum_phase[j];
        }

        self.first_frame = false;

        // --- Identity phase locking ---
        // Find the nearest peak for each bin
        // A peak is a local maximum in the shifted magnitude spectrum
        self.find_peak_regions();

        // Lock non-peak bins to their nearest peak's phase, preserving the
        // original phase offset relative to that peak (with interpolated lookup)
        for j in 0..NUM_BINS {
            let p = self.peak_bin[j];
            if p != j {
                let src_j = j as f64 / self.ratio;
                let sj = (src_j.floor() as usize).min(NUM_BINS - 2);
                let tj = src_j - sj as f64;

                let src_p = p as f64 / self.ratio;
                let sp = (src_p.floor() as usize).min(NUM_BINS - 2);
                let tp = src_p - sp as f64;

                let ph_j = lerp_phase(self.analysis_phase[sj], self.analysis_phase[sj + 1], tj);
                let ph_p = lerp_phase(self.analysis_phase[sp], self.analysis_phase[sp + 1], tp);

                self.shifted_phase[j] = self.shifted_phase[p] + (ph_j - ph_p);
            }
        }

        // --- Build output spectrum ---
        self.shifted.fill(Complex::new(0.0, 0.0));
        for j in 0..NUM_BINS {
            let mag = self.shifted_mag[j];
            if mag > 0.0 {
                let ph = self.shifted_phase[j];
                self.shifted[j] = Complex::new((mag * ph.cos()) as f32, (mag * ph.sin()) as f32);
            }
        }

        self.shifted[0].im = 0.0;
        self.shifted[NUM_BINS - 1].im = 0.0;

        // --- Synthesis ---
        self.c2r
            .process_with_scratch(
                &mut self.shifted,
                &mut self.synth_frame,
                &mut self.c2r_scratch,
            )
            .expect("IFFT failed");

        let scale = self.output_scale;
        for i in 0..FFT_SIZE {
            let pos = (self.output_write + i) % OUTPUT_SIZE;
            self.output_accum[pos] += self.synth_frame[i] * self.window[i] * scale;
        }
        self.output_write = (self.output_write + HOP_SIZE) % OUTPUT_SIZE;
    }

    /// Assign each bin to its owning spectral peak using region-of-influence.
    ///
    /// Instead of assigning to the nearest peak by distance (which can cross
    /// spectral valleys), this finds the magnitude valley between each pair of
    /// adjacent peaks and splits ownership there. Bins within a peak's region
    /// share coherent phase relationships, reducing "spacey" smearing.
    fn find_peak_regions(&mut self) {
        let max_mag = self
            .shifted_mag
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(1e-12);
        let thresh = max_mag * 0.02;

        // Collect peak indices (reuse struct-owned Vec to avoid RT allocation)
        self.peaks.clear();
        self.peaks.push(0); // DC
        for j in 1..NUM_BINS - 1 {
            if self.shifted_mag[j] > thresh
                && self.shifted_mag[j] >= self.shifted_mag[j - 1]
                && self.shifted_mag[j] >= self.shifted_mag[j + 1]
            {
                self.peaks.push(j);
            }
        }
        self.peaks.push(NUM_BINS - 1); // Nyquist

        // For each pair of adjacent peaks, find the valley (magnitude minimum)
        // between them and split ownership there
        for w in self.peaks.windows(2) {
            let left = w[0];
            let right = w[1];

            // Find the bin with minimum magnitude between the two peaks
            let mut valley = left;
            let mut min_mag = f64::MAX;
            for j in left..=right {
                if self.shifted_mag[j] < min_mag {
                    min_mag = self.shifted_mag[j];
                    valley = j;
                }
            }

            // Left peak owns bins up to and including the valley
            for j in left..=valley {
                self.peak_bin[j] = left;
            }
            // Right peak owns bins after the valley
            for j in (valley + 1)..=right {
                self.peak_bin[j] = right;
            }
        }
    }
}
