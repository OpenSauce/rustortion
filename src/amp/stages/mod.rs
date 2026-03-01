pub mod clipper;
pub mod common;
pub mod compressor;
pub mod delay;
pub mod filter;
pub mod level;
pub mod multiband_saturator;
pub mod noise_gate;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

// The core trait that all processing stages must implement
pub trait Stage: Send + Sync + 'static {
    // Process a single sample through this stage
    fn process(&mut self, input: f32) -> f32;

    // Process a block of samples through this stage
    fn process_block(&mut self, input: &mut [f32]) {
        for sample in input.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    // Set a parameter value by name
    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str>;

    // Get a parameter value by name
    fn get_parameter(&self, name: &str) -> Result<f32, &'static str>;
}
