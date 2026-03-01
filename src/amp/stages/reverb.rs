use crate::amp::stages::Stage;

// Freeverb tuning constants (reference values at 44100 Hz)
const COMB_DELAYS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_DELAYS: [usize; 4] = [556, 441, 341, 225];
const REFERENCE_SAMPLE_RATE: f32 = 44100.0;

const SCALE_ROOM: f32 = 0.28;
const OFFSET_ROOM: f32 = 0.7;
const SCALE_DAMP: f32 = 0.4;
const INPUT_GAIN: f32 = 0.015;
const ALLPASS_FEEDBACK: f32 = 0.5;

const DENORMAL_THRESHOLD: f32 = 1e-20;

/// Lowpass-feedback comb filter used in Freeverb.
struct CombFilter {
    buffer: Vec<f32>,
    write_pos: usize,
    filterstore: f32,
    feedback: f32,
    damp1: f32,
    damp2: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            write_pos: 0,
            filterstore: 0.0,
            feedback: 0.0,
            damp1: 0.0,
            damp2: 1.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.write_pos];

        // One-pole lowpass in feedback path
        self.filterstore = self.damp2.mul_add(output, self.damp1 * self.filterstore);
        if self.filterstore.abs() < DENORMAL_THRESHOLD {
            self.filterstore = 0.0;
        }

        self.buffer[self.write_pos] = self.feedback.mul_add(self.filterstore, input);

        self.write_pos += 1;
        if self.write_pos >= self.buffer.len() {
            self.write_pos = 0;
        }

        output
    }

    const fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback;
    }

    const fn set_damp(&mut self, damp1: f32, damp2: f32) {
        self.damp1 = damp1;
        self.damp2 = damp2;
    }
}

/// Allpass filter used in Freeverb with fixed coefficient of 0.5.
struct AllpassFilter {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            write_pos: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buffer[self.write_pos];
        let output = bufout - input;

        self.buffer[self.write_pos] = ALLPASS_FEEDBACK.mul_add(bufout, input);

        self.write_pos += 1;
        if self.write_pos >= self.buffer.len() {
            self.write_pos = 0;
        }

        output
    }
}

/// Scale a reference delay length (at 44100 Hz) to the actual sample rate.
fn scale_delay(reference_len: usize, sample_rate: f32) -> usize {
    (reference_len as f32 * sample_rate / REFERENCE_SAMPLE_RATE)
        .round()
        .max(1.0) as usize
}

/// Freeverb reverb stage.
///
/// Uses 8 parallel lowpass-feedback comb filters summed into 4 series
/// allpass filters (Schroeder-Moorer architecture).
pub struct ReverbStage {
    combs: [CombFilter; 8],
    allpasses: [AllpassFilter; 4],
    room_size: f32,
    damping: f32,
    mix: f32,
}

impl ReverbStage {
    pub fn new(room_size: f32, damping: f32, mix: f32, sample_rate: f32) -> Self {
        let room_size = room_size.clamp(0.0, 1.0);
        let damping = damping.clamp(0.0, 1.0);
        let mix = mix.clamp(0.0, 1.0);

        let combs = COMB_DELAYS.map(|d| CombFilter::new(scale_delay(d, sample_rate)));
        let allpasses = ALLPASS_DELAYS.map(|d| AllpassFilter::new(scale_delay(d, sample_rate)));

        let mut stage = Self {
            combs,
            allpasses,
            room_size,
            damping,
            mix,
        };
        stage.update_combs();
        stage
    }

    fn update_combs(&mut self) {
        let feedback = self.room_size.mul_add(SCALE_ROOM, OFFSET_ROOM);
        let damp1 = self.damping * SCALE_DAMP;
        let damp2 = 1.0 - damp1;

        for comb in &mut self.combs {
            comb.set_feedback(feedback);
            comb.set_damp(damp1, damp2);
        }
    }
}

impl Stage for ReverbStage {
    fn process(&mut self, input: f32) -> f32 {
        let scaled_input = input * INPUT_GAIN;

        // Sum 8 parallel comb filters
        let mut out = 0.0_f32;
        for comb in &mut self.combs {
            out += comb.process(scaled_input);
        }

        // Pass through 4 series allpass filters
        for allpass in &mut self.allpasses {
            out = allpass.process(out);
        }

        // Flush denormals
        if out.abs() < DENORMAL_THRESHOLD {
            out = 0.0;
        }

        // Dry/wet mix
        (1.0 - self.mix).mul_add(input, self.mix * out)
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "room_size" => {
                if (0.0..=1.0).contains(&value) {
                    self.room_size = value;
                    self.update_combs();
                    Ok(())
                } else {
                    Err("Room size must be between 0.0 and 1.0")
                }
            }
            "damping" => {
                if (0.0..=1.0).contains(&value) {
                    self.damping = value;
                    self.update_combs();
                    Ok(())
                } else {
                    Err("Damping must be between 0.0 and 1.0")
                }
            }
            "mix" => {
                if (0.0..=1.0).contains(&value) {
                    self.mix = value;
                    Ok(())
                } else {
                    Err("Mix must be between 0.0 and 1.0")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "room_size" => Ok(self.room_size),
            "damping" => Ok(self.damping),
            "mix" => Ok(self.mix),
            _ => Err("Unknown parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    #[test]
    fn dry_passthrough() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 0.0, SAMPLE_RATE);
        for i in 0..1000 {
            let input = (i as f32) * 0.001;
            let output = reverb.process(input);
            assert!(
                (output - input).abs() < 1e-6,
                "Expected dry passthrough at sample {i}, got {output}"
            );
        }
    }

    #[test]
    fn silence_in_silence_out() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 1.0, SAMPLE_RATE);
        for _ in 0..10000 {
            let output = reverb.process(0.0);
            assert!(output.abs() < 1e-10, "Expected silence, got {output}");
        }
    }

    #[test]
    fn impulse_produces_output() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 1.0, SAMPLE_RATE);

        // Send impulse
        let _ = reverb.process(1.0);

        // Collect output for a while — reverb tail should be audible
        let mut max_out: f32 = 0.0;
        for _ in 0..SAMPLE_RATE as usize {
            let out = reverb.process(0.0);
            max_out = max_out.max(out.abs());
        }

        assert!(
            max_out > 0.001,
            "Reverb should produce audible output after impulse, got max {max_out}"
        );
    }

    #[test]
    fn reverb_tail_decays() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 1.0, SAMPLE_RATE);

        // Send impulse
        let _ = reverb.process(1.0);

        // Measure energy in two consecutive windows
        let window_size = SAMPLE_RATE as usize / 4;
        let mut energy_first = 0.0_f32;
        for _ in 0..window_size {
            let out = reverb.process(0.0);
            energy_first += out * out;
        }

        let mut energy_second = 0.0_f32;
        for _ in 0..window_size {
            let out = reverb.process(0.0);
            energy_second += out * out;
        }

        assert!(
            energy_second < energy_first,
            "Reverb tail should decay: second window energy {energy_second} >= first {energy_first}"
        );
    }

    #[test]
    fn room_size_affects_decay() {
        // Larger room = longer decay (more feedback)
        let mut small_room = ReverbStage::new(0.1, 0.5, 1.0, SAMPLE_RATE);
        let mut large_room = ReverbStage::new(0.9, 0.5, 1.0, SAMPLE_RATE);

        // Send impulse to both
        let _ = small_room.process(1.0);
        let _ = large_room.process(1.0);

        // Measure energy after some time
        let skip = SAMPLE_RATE as usize / 2;
        let measure = SAMPLE_RATE as usize / 4;

        for _ in 0..skip {
            small_room.process(0.0);
            large_room.process(0.0);
        }

        let mut energy_small = 0.0_f32;
        let mut energy_large = 0.0_f32;
        for _ in 0..measure {
            let s = small_room.process(0.0);
            let l = large_room.process(0.0);
            energy_small += s * s;
            energy_large += l * l;
        }

        assert!(
            energy_large > energy_small,
            "Larger room should have more energy late in tail: large={energy_large}, small={energy_small}"
        );
    }

    #[test]
    fn damping_affects_brightness() {
        // Higher damping = less high frequency content
        // We test by comparing zero-crossing rates (brighter = more crossings)
        let mut low_damp = ReverbStage::new(0.5, 0.1, 1.0, SAMPLE_RATE);
        let mut high_damp = ReverbStage::new(0.5, 0.9, 1.0, SAMPLE_RATE);

        // Send impulse
        let _ = low_damp.process(1.0);
        let _ = high_damp.process(1.0);

        // Count zero crossings in the tail
        let num_samples = SAMPLE_RATE as usize;
        let mut prev_low = 0.0_f32;
        let mut prev_high = 0.0_f32;
        let mut crossings_low: usize = 0;
        let mut crossings_high: usize = 0;

        for _ in 0..num_samples {
            let l = low_damp.process(0.0);
            let h = high_damp.process(0.0);

            if l * prev_low < 0.0 {
                crossings_low += 1;
            }
            if h * prev_high < 0.0 {
                crossings_high += 1;
            }

            prev_low = l;
            prev_high = h;
        }

        assert!(
            crossings_low > crossings_high,
            "Low damping should have more zero crossings (brighter): low={crossings_low}, high={crossings_high}"
        );
    }

    #[test]
    fn parameter_validation() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 0.5, SAMPLE_RATE);

        assert!(reverb.set_parameter("room_size", -0.1).is_err());
        assert!(reverb.set_parameter("room_size", 1.1).is_err());
        assert!(reverb.set_parameter("room_size", 0.5).is_ok());

        assert!(reverb.set_parameter("damping", -0.1).is_err());
        assert!(reverb.set_parameter("damping", 1.1).is_err());
        assert!(reverb.set_parameter("damping", 0.5).is_ok());

        assert!(reverb.set_parameter("mix", -0.1).is_err());
        assert!(reverb.set_parameter("mix", 1.1).is_err());
        assert!(reverb.set_parameter("mix", 0.5).is_ok());

        assert!(reverb.set_parameter("unknown", 0.0).is_err());
    }

    #[test]
    fn constructor_clamps_out_of_range() {
        let reverb = ReverbStage::new(2.0, 2.0, 2.0, SAMPLE_RATE);
        assert!((reverb.get_parameter("room_size").unwrap() - 1.0).abs() < 1e-6);
        assert!((reverb.get_parameter("damping").unwrap() - 1.0).abs() < 1e-6);
        assert!((reverb.get_parameter("mix").unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn get_set_parameter_roundtrip() {
        let mut reverb = ReverbStage::new(0.5, 0.5, 0.5, SAMPLE_RATE);

        reverb.set_parameter("room_size", 0.7).unwrap();
        assert!((reverb.get_parameter("room_size").unwrap() - 0.7).abs() < 1e-6);

        reverb.set_parameter("damping", 0.3).unwrap();
        assert!((reverb.get_parameter("damping").unwrap() - 0.3).abs() < 1e-6);

        reverb.set_parameter("mix", 0.8).unwrap();
        assert!((reverb.get_parameter("mix").unwrap() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn output_stays_bounded() {
        let mut reverb = ReverbStage::new(0.9, 0.5, 1.0, SAMPLE_RATE);

        // Feed sustained signal for 2 seconds
        let mut max_out: f32 = 0.0;
        for _ in 0..(SAMPLE_RATE as usize * 2) {
            let out = reverb.process(0.5);
            max_out = max_out.max(out.abs());
        }

        assert!(
            max_out < 5.0,
            "Output should stay bounded with sustained input, got max {max_out}"
        );
    }

    #[test]
    fn different_sample_rates_produce_valid_output() {
        for &rate in &[44100.0, 48000.0, 96000.0] {
            let mut reverb = ReverbStage::new(0.5, 0.5, 1.0, rate);

            // Send impulse
            let _ = reverb.process(1.0);

            let mut max_out: f32 = 0.0;
            for _ in 0..rate as usize {
                let out = reverb.process(0.0);
                max_out = max_out.max(out.abs());
            }

            assert!(
                max_out > 0.001,
                "Reverb should produce output at sample rate {rate}, got max {max_out}"
            );
        }
    }
}
