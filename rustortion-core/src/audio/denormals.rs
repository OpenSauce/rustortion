//! CPU denormal handling for the real-time audio path.
//!
//! Denormal (subnormal) floats are handled by microcode on most x86 CPUs and cost on the order of
//! 100x a normal multiply. They show up naturally in this chain: IIR filter state in
//! [`crate::amp::stages::filter`] and any feedback/decay path keep multiplying their own output by
//! a coefficient below 1.0, so as soon as the guitar signal stops the state slides down through the
//! denormal range instead of reaching zero. The result is a CPU spike precisely when the DSP is
//! doing nothing useful.
//!
//! Rather than sprinkling per-stage denormal guards (adding a tiny DC offset, or clamping state)
//! through every filter and delay, we flip the CPU's own denormal-control bits once at the entry
//! point of the audio callback. That covers the whole chain, including third-party DSP, for a
//! handful of cycles.
//!
//! # Per-thread state
//!
//! **MXCSR (x86) and FPCR (`AArch64`) are per-thread register state.** Setting them on one thread
//! says nothing about any other thread. That is why [`enable_flush_to_zero`] is called at the top
//! of *every* process callback rather than once when the audio thread starts:
//!
//! * JACK, and plugin hosts generally, are free to hand us a different thread — after a graph
//!   reorder, a freewheel/export pass, or a thread-pool change — with no notification.
//! * Some hosts and drivers reset or restore MXCSR around their own code.
//! * The cost is a register read plus a conditional register write, which is negligible next to a
//!   whole block of amp simulation.
//!
//! Unlike nih-plug's `ScopedFtz`, we do not restore the previous value on exit. The standalone JACK
//! process thread is ours for its entire lifetime and runs nothing but this chain, so there is
//! nobody to restore it for.
//!
//! # Real-time safety
//!
//! No allocation, no locks, no I/O, no logging — safe to call from the audio callback.

/// Flush-to-zero: MXCSR bit 15. Denormal *results* are flushed to zero.
///
/// See Intel SDM Vol. 1 section 10.2.3.3 (Flush-To-Zero).
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse"
))]
const MXCSR_FLUSH_TO_ZERO: u32 = 1 << 15;

/// Denormals-are-zero: MXCSR bit 6. Denormal *inputs* are treated as zero.
///
/// This bit is an SSE2 addition (Intel SDM Vol. 1 section 10.2.3.4). It is unconditionally
/// available on `x86_64`, but on 32-bit x86 a pre-SSE2 CPU treats bit 6 as reserved and writing it
/// raises `#GP`, so it is gated on the `sse2` target feature.
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2"
))]
const MXCSR_DENORMALS_ARE_ZERO: u32 = 1 << 6;

/// `AArch64` flush-to-zero: FPCR bit 24 (`FZ`).
///
/// See the Arm Architecture Reference Manual, FPCR (Floating-point Control Register).
#[cfg(target_arch = "aarch64")]
const FPCR_FLUSH_TO_ZERO: u64 = 1 << 24;

/// Enables flush-to-zero (and, where available, denormals-are-zero) for the **calling thread**.
///
/// Call this at the top of every real-time audio callback. See the [module docs](self) for why it
/// is per-callback rather than once per thread, and for the real-time safety guarantees.
///
/// On architectures without a denormal-control register we know about, this is a no-op.
#[inline]
pub fn enable_flush_to_zero() {
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse"
    ))]
    {
        // `_mm_getcsr`/`_mm_setcsr` and the `_MM_SET_FLUSH_ZERO_MODE`/`_MM_SET_DENORMALS_ZERO_MODE`
        // wrappers were deprecated in Rust 1.75, so the register has to be touched via inline asm.
        let mut wanted = MXCSR_FLUSH_TO_ZERO;
        #[cfg(target_feature = "sse2")]
        {
            wanted |= MXCSR_DENORMALS_ARE_ZERO;
        }

        let mut mxcsr: u32 = 0;
        // SAFETY: `stmxcsr` stores the 32-bit MXCSR register through the given pointer, which
        // points at an initialised, correctly aligned, exclusively borrowed `u32` on our stack.
        unsafe {
            core::arch::asm!("stmxcsr [{}]", in(reg) &raw mut mxcsr, options(nostack, preserves_flags));
        }

        let updated = mxcsr | wanted;
        if updated != mxcsr {
            // SAFETY: `ldmxcsr` loads MXCSR from the given pointer, which points at an initialised,
            // correctly aligned `u32` on our stack. `updated` only ever sets bits 15 and (when
            // SSE2 is available) 6, both of which are defined and writable, so no reserved bit is
            // set and this cannot raise `#GP`.
            unsafe {
                core::arch::asm!("ldmxcsr [{}]", in(reg) &raw const updated, options(nostack, preserves_flags));
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // There are no stable intrinsics for FPCR, so this needs inline asm too.
        let fpcr: u64;
        // SAFETY: `mrs`/`msr` on FPCR are unprivileged and always available on AArch64. Reading
        // and writing the calling thread's own floating-point control register has no effect on
        // memory or on any other thread.
        unsafe {
            core::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
        }

        let updated = fpcr | FPCR_FLUSH_TO_ZERO;
        if updated != fpcr {
            // SAFETY: see above; only the defined `FZ` bit is being set.
            unsafe {
                core::arch::asm!("msr fpcr, {}", in(reg) updated, options(nomem, nostack, preserves_flags));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::enable_flush_to_zero;
    use std::hint::black_box;

    /// Smallest positive denormal `f32`. Halving `f32::MIN_POSITIVE` lands in denormal range.
    const DENORMAL: f32 = f32::MIN_POSITIVE / 2.0;

    #[test]
    fn is_callable_and_idempotent() {
        enable_flush_to_zero();
        enable_flush_to_zero();
    }

    /// Sanity check that the constant we build the test around really is subnormal, and that
    /// without FTZ the multiply below would produce a (nonzero) denormal.
    #[test]
    fn denormal_constant_is_subnormal() {
        assert!(black_box(DENORMAL).is_subnormal());
    }

    /// Multiplying a denormal by a normal value yields a denormal result, which FTZ must flush to
    /// exactly zero.
    ///
    /// `black_box` on both operands is load-bearing: without it the optimiser is free to constant
    /// fold this at compile time, where FTZ does not apply, and the test would pass or fail for
    /// reasons unrelated to the runtime MXCSR/FPCR state.
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse"
        ),
        target_arch = "aarch64"
    ))]
    #[test]
    fn denormal_result_flushes_to_zero() {
        enable_flush_to_zero();

        let product = black_box(DENORMAL) * black_box(0.5_f32);
        assert_eq!(black_box(product), 0.0_f32);
    }

    /// On x86 with SSE2, DAZ additionally makes a denormal *input* read as zero, so adding a
    /// denormal to zero also gives exactly zero rather than the denormal itself.
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2"
    ))]
    #[test]
    fn denormal_input_reads_as_zero() {
        enable_flush_to_zero();

        let sum = black_box(DENORMAL) + black_box(0.0_f32);
        assert_eq!(black_box(sum), 0.0_f32);
    }
}
