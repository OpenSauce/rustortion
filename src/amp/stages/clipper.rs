use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::sync::LazyLock;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipperType {
    Soft,       // Smooth, tube-like saturation (similar to Tanh)
    Medium,     // Balanced clipping (similar to ArcTan)
    Hard,       // More aggressive clipping (similar to HardClip)
    Asymmetric, // Tube-like even harmonic generation
    ClassA,     // Classic Class A tube preamp behavior
    Triode,     // 12AX7 triode model via lookup table
}

impl std::fmt::Display for ClipperType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Soft => write!(f, "{}", crate::tr!(clipper_soft)),
            Self::Medium => write!(f, "{}", crate::tr!(clipper_medium)),
            Self::Hard => write!(f, "{}", crate::tr!(clipper_hard)),
            Self::Asymmetric => write!(f, "{}", crate::tr!(clipper_asymmetric)),
            Self::ClassA => write!(f, "{}", crate::tr!(clipper_class_a)),
            Self::Triode => write!(f, "{}", crate::tr!(clipper_triode)),
        }
    }
}

impl ClipperType {
    #[inline]
    pub fn process(&self, input: f32, drive: f32) -> f32 {
        let driven = input * drive;

        match self {
            Self::Soft => {
                // Soft clipping using tanh for smooth tube-like saturation
                driven.tanh()
            }

            Self::Medium => {
                // Medium clipping using arctan for a balanced distortion
                driven.atan() * (2.0 / PI)
            }

            Self::Hard => {
                // Hard clipping with sharp cutoff
                driven.clamp(-1.0, 1.0)
            }

            Self::Asymmetric => {
                // Asymmetric clipping to model even harmonics from tubes
                // Positive signals clip differently than negative ones
                if driven >= 0.0 {
                    driven.tanh()
                } else {
                    0.7f32.mul_add(driven.tanh(), 0.3 * driven)
                }
            }

            Self::ClassA => {
                // Class A tube preamp behavior
                // Combines soft clipping with subtle wave folding for complex harmonics
                let soft_clip = driven.tanh();
                let fold_amount: f32 = 0.3;
                let folded = if driven.abs() > 1.0 {
                    let fold_factor = 2.0 - driven.abs().min(2.0);
                    soft_clip * fold_factor
                } else {
                    soft_clip
                };

                (1.0 - fold_amount).mul_add(soft_clip, fold_amount * folded)
            }

            Self::Triode => TRIODE_TABLE.process(driven),
        }
    }
}

/// Pre-computed transfer curve for a 12AX7 triode using the Koren model.
///
/// The table is filled at init time from the Koren plate current equation
/// (Norman Koren, "Improved Vacuum Tube Models for SPICE Simulations")
/// and looked up at runtime via cubic Hermite (Catmull-Rom) interpolation.
struct TubeTable {
    table: [f32; Self::SIZE],
    input_min: f32,
    input_max: f32,
}

impl TubeTable {
    const SIZE: usize = 256;
}

impl TubeTable {
    fn new() -> Self {
        // Koren 12AX7 triode parameters:
        const MU: f64 = 100.0; // amplification factor
        const KP: f64 = 600.0; // plate current coefficient
        const KVB: f64 = 300.0; // knee voltage coefficient
        const KG1: f64 = 1060.0; // grid current scaling
        const EX: f64 = 1.4; // plate current exponent
        const VP: f64 = 250.0; // plate voltage (operating point)
        const VG_SCALE: f64 = 4.0; // maps normalized input to grid voltage range

        let input_min: f64 = -5.0;
        let input_max: f64 = 5.0;

        let mut raw = [0.0f64; Self::SIZE];
        for (idx, sample) in raw.iter_mut().enumerate() {
            let t = idx as f64 / (Self::SIZE - 1) as f64;
            let input = t.mul_add(input_max - input_min, input_min);
            let vg = input * VG_SCALE;

            // Koren equation: E1 = (Vp/Kp) * ln(1 + exp(Kp * (1/mu + Vg/sqrt(Kvb + Vp²))))
            let inner = KP * (1.0 / MU + vg / VP.mul_add(VP, KVB).sqrt());
            let inner_clamped = inner.min(80.0);
            let e1 = inner_clamped.exp().ln_1p() * VP / KP;
            let e1_safe = e1.max(0.0);
            // Plate current: Ip = E1^Ex / Kg1
            *sample = e1_safe.powf(EX) / KG1;
        }

        let ip_min = raw.iter().copied().fold(f64::INFINITY, f64::min);
        let ip_max = raw.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = ip_max - ip_min;

        let mut table = [0.0f32; Self::SIZE];
        for (i, &ip) in raw.iter().enumerate() {
            table[i] = if range > 0.0 {
                (2.0 * (ip - ip_min) / range - 1.0) as f32
            } else {
                0.0
            };
        }

        Self {
            table,
            input_min: input_min as f32,
            input_max: input_max as f32,
        }
    }

    #[inline]
    fn process(&self, input: f32) -> f32 {
        let clamped = input.clamp(self.input_min, self.input_max);
        let normalized = (clamped - self.input_min) / (self.input_max - self.input_min);
        let index_f = normalized * (self.table.len() - 1) as f32;
        let idx = (index_f as usize).min(self.table.len() - 2);
        let frac = index_f - idx as f32;

        let p0 = self.table[idx.saturating_sub(1)];
        let p1 = self.table[idx];
        let p2 = self.table[(idx + 1).min(self.table.len() - 1)];
        let p3 = self.table[(idx + 2).min(self.table.len() - 1)];

        let coeff_a = (-0.5f32).mul_add(p0, 1.5 * p1) + (-1.5f32).mul_add(p2, 0.5 * p3);
        let coeff_b = 2.5f32.mul_add(-p1, p0) + 2.0f32.mul_add(p2, -0.5 * p3);
        let coeff_c = (-0.5f32).mul_add(p0, 0.5 * p2);

        coeff_a
            .mul_add(frac, coeff_b)
            .mul_add(frac, coeff_c)
            .mul_add(frac, p1)
    }
}

static TRIODE_TABLE: LazyLock<TubeTable> = LazyLock::new(TubeTable::new);

/// Force initialization of the triode lookup table.
/// Call during app startup to avoid lazy init on the RT audio thread.
pub fn init() {
    LazyLock::force(&TRIODE_TABLE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triode_zero_input() {
        let output = ClipperType::Triode.process(0.0, 1.0);
        assert!(
            output.is_finite() && (-1.1..=1.1).contains(&output),
            "expected finite bounded output for zero input, got {output}"
        );
    }

    #[test]
    fn test_triode_bounded_output() {
        for drive in [1.0, 3.0, 5.0, 10.0] {
            for i in -20..=20 {
                let input = i as f32 * 0.1;
                let output = ClipperType::Triode.process(input, drive);
                assert!(
                    (-1.1..=1.1).contains(&output),
                    "output {output} out of bounds for input={input}, drive={drive}"
                );
            }
        }
    }

    #[test]
    fn test_triode_asymmetric() {
        let pos = ClipperType::Triode.process(0.5, 1.0);
        let neg = ClipperType::Triode.process(-0.5, 1.0);
        assert!(
            (pos.abs() - neg.abs()).abs() > 1e-6,
            "expected asymmetric response, got pos={pos}, neg={neg}"
        );
    }

    #[test]
    fn test_triode_monotonic() {
        let mut prev = ClipperType::Triode.process(-2.0, 1.0);
        for i in -19..=20 {
            let input = i as f32 * 0.1;
            let output = ClipperType::Triode.process(input, 1.0);
            assert!(
                output >= prev - 1e-6,
                "non-monotonic at input={input}: prev={prev}, current={output}"
            );
            prev = output;
        }
    }

    #[test]
    fn test_triode_table_normalized() {
        let table = &TRIODE_TABLE.table;
        let min = table.iter().copied().fold(f32::INFINITY, f32::min);
        let max = table.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (min - (-1.0)).abs() < 0.05,
            "table min should be near -1.0, got {min}"
        );
        assert!(
            (max - 1.0).abs() < 0.05,
            "table max should be near 1.0, got {max}"
        );
    }
}
