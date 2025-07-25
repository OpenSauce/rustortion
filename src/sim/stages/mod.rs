pub mod clipper;
pub mod compressor;
pub mod filter;
pub mod level;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

// The core trait that all processing stages must implement
pub trait Stage: Send + Sync + 'static {
    // Process a single sample through this stage
    fn process(&mut self, input: f32) -> f32;

    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str>;

    // Get a parameter value by name
    fn get_parameter(&self, name: &str) -> Result<f32, &'static str>;

    // Get the name of this stage
    fn name(&self) -> &str;
}
