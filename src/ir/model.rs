use rustfft::num_complex::Complex;

#[derive(Clone)]
pub struct ImpulseResponse {
    pub head_coeffs: Vec<f32>,

    pub tail_partitions: Vec<Vec<Complex<f32>>>,
    pub num_tail_partitions: usize,

    pub original_length: usize,
}
