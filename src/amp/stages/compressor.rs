use crate::amp::stages::Stage;
use crate::amp::stages::common::{EnvelopeFollower, calculate_coefficient, db_to_lin};

pub struct CompressorStage {
    attack_ms: f32,  // Attack time in milliseconds
    release_ms: f32, // Release time in milliseconds
    threshold: f32,  // Threshold in linear scale
    ratio: f32,      // Compression ratio (e.g., 4.0 for 4:1)
    makeup: f32,     // Makeup gain in linear scale
    envelope: EnvelopeFollower,
    sample_rate: f32,
}

impl CompressorStage {
    pub fn new(
        attack_ms: f32,
        release_ms: f32,
        threshold_db: f32,
        ratio: f32,
        makeup_db: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            attack_ms,
            release_ms,
            threshold: db_to_lin(threshold_db),
            ratio,
            makeup: db_to_lin(makeup_db),
            envelope: EnvelopeFollower::from_ms(attack_ms, release_ms, sample_rate),
            sample_rate,
        }
    }

    fn update_attack(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms;
        self.envelope
            .set_attack_coeff(calculate_coefficient(attack_ms, self.sample_rate));
    }

    fn update_release(&mut self, release_ms: f32) {
        self.release_ms = release_ms;
        self.envelope
            .set_release_coeff(calculate_coefficient(release_ms, self.sample_rate));
    }
}

impl Stage for CompressorStage {
    fn process(&mut self, input: f32) -> f32 {
        // Envelope follower (feed abs input, avoid log(0))
        let level_in = input.abs().max(1e-10);
        self.envelope.process(level_in);
        let env = self.envelope.value();

        // Compression gain calculation
        let over_threshold = (env / self.threshold).max(1.0);
        let gain_reduction = if over_threshold > 1.0 {
            over_threshold.powf((1.0 / self.ratio) - 1.0)
        } else {
            1.0
        };

        input * gain_reduction * self.makeup
    }

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str> {
        match name {
            "threshold" => {
                if (-60.0..=0.).contains(&value) {
                    self.threshold = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Threshold must be between -60 dB and 0 dB")
                }
            }
            "ratio" => {
                if (1.0..=20.0).contains(&value) {
                    self.ratio = value;
                    Ok(())
                } else {
                    Err("Ratio must be between 1.0 and 20.0")
                }
            }
            "attack" => {
                if (0.1..=100.0).contains(&value) {
                    self.update_attack(value);
                    Ok(())
                } else {
                    Err("Attack must be between 0.1 ms and 100 ms")
                }
            }
            "release" => {
                if (10.0..=1000.0).contains(&value) {
                    self.update_release(value);
                    Ok(())
                } else {
                    Err("Release must be between 10 ms and 1000 ms")
                }
            }
            "makeup" => {
                if (-12.0..=24.0).contains(&value) {
                    self.makeup = db_to_lin(value);
                    Ok(())
                } else {
                    Err("Makeup must be between -12 dB and 24 dB")
                }
            }
            _ => Err("Unknown parameter"),
        }
    }

    fn get_parameter(&self, name: &str) -> Result<f32, &'static str> {
        match name {
            "threshold" => Ok(20.0 * self.threshold.log10()),
            "ratio" => Ok(self.ratio),
            "attack" => Ok(self.attack_ms),
            "release" => Ok(self.release_ms),
            "makeup" => Ok(20.0 * self.makeup.log10()),
            _ => Err("Unknown parameter"),
        }
    }
}
