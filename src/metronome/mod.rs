pub struct Metronome {
    bpm: f32,
    sample_rate: usize,
    samples_per_beat: usize,
    sample_counter: usize,
    enabled: bool,
}

impl Metronome {
    pub fn new(bpm: f32, sample_rate: usize) -> Self {
        let samples_per_beat = (sample_rate as f32 * 60.0 / bpm) as usize;

        // load wav file

        Self {
            bpm,
            sample_rate,
            samples_per_beat,
            sample_counter: 0,
            enabled: false,
        }
    }

    pub fn process_block(&mut self, output: &mut [f32]) {
        for sample in output.iter_mut() {
            if self.sample_counter == 0 {
                *sample = 1.0; // Metronome click
            // use wav file
            } else {
                *sample = 0.0; // Silence
            }

            self.sample_counter += 1;
            if self.sample_counter >= self.samples_per_beat {
                self.sample_counter = 0;
            }
        }
    }

    pub fn start(bpm: f32) {}
}
