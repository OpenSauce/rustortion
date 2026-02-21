use std::f64::consts::PI;
use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

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

        // Hann² with 87.5% overlap sums to exactly 3.0 at every sample
        let output_scale = 1.0 / (FFT_SIZE as f32 * 3.0);

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
        }
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
            let src_k = source as usize;
            if src_k >= NUM_BINS - 1 {
                continue;
            }

            let frac = source - src_k as f64;

            let mag = self.analysis_mag[src_k]
                + (self.analysis_mag[src_k + 1] - self.analysis_mag[src_k]) * frac;
            let freq = (self.analysis_freq[src_k]
                + (self.analysis_freq[src_k + 1] - self.analysis_freq[src_k]) * frac)
                * self.ratio;

            self.accum_phase[j] += freq;
            self.shifted_mag[j] = mag;
            self.shifted_phase[j] = self.accum_phase[j];
        }

        // Preserve spectral energy: bin shifting loses energy from truncated
        // bins, interpolation smoothing, and phase imperfections. Scale the
        // output magnitudes so total energy matches the input.
        let input_energy: f64 = self.analysis_mag.iter().map(|&m| m * m).sum();
        let output_energy: f64 = self.shifted_mag.iter().map(|&m| m * m).sum();
        if output_energy > 1e-20 {
            let gain = (input_energy / output_energy).sqrt();
            for m in &mut self.shifted_mag {
                *m *= gain;
            }
        }

        // --- Identity phase locking ---
        // Find the nearest peak for each bin
        // A peak is a local maximum in the shifted magnitude spectrum
        self.find_nearest_peaks();

        // Lock non-peak bins to their nearest peak's phase, preserving the
        // original phase offset relative to that peak
        for j in 0..NUM_BINS {
            let p = self.peak_bin[j];
            if p != j {
                // Phase offset this bin had relative to the peak in the analysis
                let src_j = j as f64 / self.ratio;
                let src_p = p as f64 / self.ratio;
                let src_j_k = (src_j as usize).min(NUM_BINS - 1);
                let src_p_k = (src_p as usize).min(NUM_BINS - 1);
                let original_offset = self.analysis_phase[src_j_k] - self.analysis_phase[src_p_k];

                self.shifted_phase[j] = self.shifted_phase[p] + original_offset;
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

    /// For each bin, find the nearest spectral peak (local magnitude maximum).
    /// Peaks are assigned to themselves; non-peak bins inherit the nearest peak.
    fn find_nearest_peaks(&mut self) {
        // Mark peaks
        let mut is_peak = [false; NUM_BINS];
        if NUM_BINS > 0 {
            is_peak[0] = true; // treat DC as its own peak
        }
        for (j, peak) in is_peak
            .iter_mut()
            .enumerate()
            .take(NUM_BINS.saturating_sub(1))
            .skip(1)
        {
            if self.shifted_mag[j] >= self.shifted_mag[j - 1]
                && self.shifted_mag[j] >= self.shifted_mag[j + 1]
            {
                *peak = true;
            }
        }
        if NUM_BINS > 1 {
            is_peak[NUM_BINS - 1] = true; // treat Nyquist as its own peak
        }

        // Forward pass: propagate nearest peak from the left
        let mut last_peak = 0;
        for (j, &peak) in is_peak.iter().enumerate().take(NUM_BINS) {
            if peak {
                last_peak = j;
            }
            self.peak_bin[j] = last_peak;
        }

        // Backward pass: pick closer peak between left and right
        last_peak = NUM_BINS - 1;
        for j in (0..NUM_BINS).rev() {
            if is_peak[j] {
                last_peak = j;
            }
            if (last_peak as isize - j as isize).unsigned_abs() < (self.peak_bin[j]).abs_diff(j) {
                self.peak_bin[j] = last_peak;
            }
        }
    }
}
