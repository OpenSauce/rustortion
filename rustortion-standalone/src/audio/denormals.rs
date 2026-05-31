//! Flush-to-zero (FTZ) setup for the real-time audio thread.
//!
//! Denormal (subnormal) floating-point arithmetic is catastrophically slow — up to
//! ~10–100× on some CPUs, and especially bad on ARM (Raspberry Pi). As signals decay
//! toward silence, the IR convolver and filter tails can drive intermediate values into
//! the denormal range, causing erratic CPU spikes that don't track IR length. Enabling
//! the CPU's flush-to-zero flag makes denormal results flush to zero, keeping cost
//! consistent.
//!
//! The VST3/CLAP plugin already gets this from nih-plug's process wrapper; the standalone
//! JACK process thread must set it itself. The flag is per-thread, so this is called from
//! inside the JACK process callback.
//!
//! The implementation mirrors nih-plug's `ScopedFtz` — Rust 1.75 deprecated the
//! `_mm_setcsr` intrinsics, so this uses inline assembly: MXCSR bit 15 on x86 SSE, FPCR
//! bit 24 on AArch64. On other targets it is a no-op.

/// Enable flush-to-zero for denormals on the current thread. Idempotent and cheap (a
/// register read plus a conditional write), so it is safe to call every process callback.
#[inline]
pub fn enable_flush_to_zero() {
    #[cfg(target_feature = "sse")]
    {
        // MXCSR bit 15 = Flush-To-Zero.
        const SSE_FTZ_BIT: u32 = 1 << 15;
        let mut mxcsr: u32 = 0;
        // SAFETY: stmxcsr/ldmxcsr only read/write the current thread's MXCSR register.
        unsafe {
            std::arch::asm!("stmxcsr [{}]", in(reg) std::ptr::addr_of_mut!(mxcsr));
            if mxcsr & SSE_FTZ_BIT == 0 {
                let updated = mxcsr | SSE_FTZ_BIT;
                std::arch::asm!("ldmxcsr [{}]", in(reg) std::ptr::addr_of!(updated));
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // FPCR bit 24 = Flush-to-zero mode.
        const AARCH64_FTZ_BIT: u64 = 1 << 24;
        let mut fpcr: u64;
        // SAFETY: FPCR is EL0-accessible; this reads then conditionally sets the FZ bit.
        unsafe {
            std::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
            if fpcr & AARCH64_FTZ_BIT == 0 {
                std::arch::asm!("msr fpcr, {}", in(reg) fpcr | AARCH64_FTZ_BIT);
            }
        }
    }
}
