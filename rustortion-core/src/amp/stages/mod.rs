pub mod clipper;
pub mod common;
pub mod compressor;
pub mod delay;
pub mod eq;
pub mod filter;
pub mod level;
pub mod multiband_saturator;
pub mod nam;
pub mod noise_gate;
pub mod poweramp;
pub mod preamp;
pub mod reverb;
pub mod tonestack;
pub mod tremolo;

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

    /// Discard every sample of accumulated state, returning the stage to the
    /// condition it was in immediately after construction — delay lines and
    /// filter memory zeroed, envelopes and LFO phase back at their initial
    /// values — while leaving all *parameters* exactly as they are.
    ///
    /// Hosts call this on transport locate/seek (via nih-plug's
    /// `Plugin::reset`), so without it a reverb tail or delay repeat from bar 32
    /// keeps ringing over bar 1.
    ///
    /// # Real-time contract
    ///
    /// This runs **on the audio thread**. Implementations must zero their
    /// buffers in place (`fill(0.0)`, resetting indices) and must never
    /// allocate, free, lock, or perform I/O. `rustortion-core/tests/no_alloc.rs`
    /// enforces this.
    ///
    /// # This is a discontinuity, by design
    ///
    /// Returning to construction state means the output *steps*. That is
    /// correct at a locate, where the input is discontinuous anyway, but
    /// nih-plug documents `Plugin::reset` as callable "at any other time from
    /// the audio thread" — landing on continuous audio, it is audible. Measured
    /// mid-signal: a dry preamp → power amp → tone stack chain steps 0.89 full
    /// scale and settles over ~59 ms, as the DC blockers restart from zero.
    /// Worst relative offender is the compressor, whose envelope drops to 0, so
    /// the first ~20 ms after a locate is **uncompressed at full makeup gain**
    /// (peak error 264% of the signal's own peak) — an overshoot into whatever
    /// follows it.
    ///
    /// Softening this (output ramp, DC-blocker priming) is deliberately not
    /// done here; it is a DSP design decision, not part of the flush contract.
    ///
    /// Required rather than defaulted on purpose: adding a stage should force
    /// the author to decide what its state is, and the compiler lists every
    /// impl that has not.
    fn reset(&mut self);

    // Set a parameter value by name
    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str>;

    // Get a parameter value by name
    fn get_parameter(&self, name: &str) -> Result<f32, &'static str>;
}
