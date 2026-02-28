use std::f32::consts::PI;

/// Convert decibels to linear amplitude.
#[inline]
pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Calculate a one-pole smoothing coefficient from a time constant in milliseconds.
///
/// Returns `exp(-1 / (sample_rate * time_ms * 0.001))`.
/// Useful for attack/release envelopes and sag filters.
#[inline]
pub fn calculate_coefficient(time_ms: f32, sample_rate: f32) -> f32 {
    (-1.0 / (sample_rate * 0.001 * time_ms)).exp()
}

/// DC blocker using a first-order high-pass filter.
///
/// `y[n] = x[n] - x[n-1] + R * y[n-1]`
///
/// Reference: <https://ccrma.stanford.edu/~jos/fp/DC_Blocker.html>
#[derive(Clone)]
pub struct DcBlocker {
    x_prev: f32,
    y_prev: f32,
    coeff: f32,
}

impl DcBlocker {
    pub fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let coeff = (-2.0 * PI * cutoff_hz / sample_rate).exp();
        Self {
            x_prev: 0.0,
            y_prev: 0.0,
            coeff,
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.coeff.mul_add(self.y_prev, input - self.x_prev);
        self.x_prev = input;
        self.y_prev = output;
        output
    }
}

/// One-pole envelope follower with configurable attack and release coefficients.
#[derive(Clone)]
pub struct EnvelopeFollower {
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl EnvelopeFollower {
    /// Create from pre-computed coefficients.
    pub const fn new(attack_coeff: f32, release_coeff: f32) -> Self {
        Self {
            envelope: 0.0,
            attack_coeff,
            release_coeff,
        }
    }

    /// Create from attack/release times in milliseconds.
    pub fn from_ms(attack_ms: f32, release_ms: f32, sample_rate: f32) -> Self {
        Self::new(
            calculate_coefficient(attack_ms, sample_rate),
            calculate_coefficient(release_ms, sample_rate),
        )
    }

    pub const fn set_attack_coeff(&mut self, coeff: f32) {
        self.attack_coeff = coeff;
    }

    pub const fn set_release_coeff(&mut self, coeff: f32) {
        self.release_coeff = coeff;
    }

    pub const fn value(&self) -> f32 {
        self.envelope
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();
        if abs_input > self.envelope {
            self.envelope = self
                .attack_coeff
                .mul_add(self.envelope, (1.0 - self.attack_coeff) * abs_input);
        } else {
            self.envelope = self
                .release_coeff
                .mul_add(self.envelope, (1.0 - self.release_coeff) * abs_input);
        }
        self.envelope
    }
}
