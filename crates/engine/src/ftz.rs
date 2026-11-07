//! Flush-to-zero guard for the audio thread.
//!
//! Subnormal floats cost tens of times a normal operation on x86 and ARM, and decaying filter,
//! delay and reverb tails produce them constantly. [`FtzGuard`] switches the CPU to flush
//! subnormal results to zero (FTZ) and treat subnormal inputs as zero (DAZ) on x86, or sets FZ
//! on aarch64, and restores the previous state when dropped, so a host's thread is never left
//! modified. Create one at the top of every audio callback, in the standalone and the CLAP plugin.
//!
//! # Undefined-behaviour caveat
//!
//! Rust documents changing the floating-point environment as undefined behaviour: LLVM assumes
//! the default environment and may constant-fold or reorder float code on that assumption. That
//! is why this uses inline asm and not `_mm_setcsr` (deprecated since Rust 1.75), and why it is
//! not a guarantee. In practice the compiler never moves float arithmetic across an opaque
//! `asm!` block in a way that matters here, and subnormal-versus-zero differences are far below
//! audibility. The guard against a miscompile is the golden renders: they are recorded from a
//! release build with FTZ on (`render_hash`, `golden_renders`), so any build whose float code
//! drifts shows up as a changed hash.

/// Sets FTZ/DAZ (x86) or FZ (aarch64) on creation and restores the previous state on drop.
/// A no-op on other architectures. Not `Send`: it restores the thread it was created on.
pub struct FtzGuard {
    saved: usize,
    _not_send: std::marker::PhantomData<*const ()>,
}

#[cfg(target_arch = "x86_64")]
mod arch {
    use std::arch::asm;
    /// MXCSR bit 15 (FTZ) and bit 6 (DAZ).
    const FTZ_DAZ: u32 = (1 << 15) | (1 << 6);

    pub fn enable() -> usize {
        let mut old: u32 = 0;
        // SAFETY: stmxcsr/ldmxcsr only read and write MXCSR through a valid stack slot.
        unsafe {
            asm!("stmxcsr [{p}]", p = in(reg) &mut old, options(nostack, preserves_flags));
            let new = old | FTZ_DAZ;
            asm!("ldmxcsr [{p}]", p = in(reg) &new, options(nostack, readonly));
        }
        old as usize
    }
    pub fn restore(saved: usize) {
        let v = saved as u32;
        // SAFETY: restores a value previously read from MXCSR.
        unsafe { asm!("ldmxcsr [{p}]", p = in(reg) &v, options(nostack, readonly)) }
    }
    pub fn current() -> usize {
        let mut v: u32 = 0;
        // SAFETY: as in `enable`.
        unsafe { asm!("stmxcsr [{p}]", p = in(reg) &mut v, options(nostack, preserves_flags)) }
        v as usize
    }
    pub fn is_on(state: usize) -> bool {
        state as u32 & FTZ_DAZ == FTZ_DAZ
    }
}

#[cfg(target_arch = "aarch64")]
mod arch {
    use std::arch::asm;
    /// FPCR bit 24 (FZ).
    const FZ: u64 = 1 << 24;

    pub fn current() -> usize {
        let v: u64;
        // SAFETY: reads FPCR.
        unsafe { asm!("mrs {v}, fpcr", v = out(reg) v, options(nomem, nostack, preserves_flags)) }
        v as usize
    }
    fn set(v: u64) {
        // SAFETY: writes FPCR with a value derived from the current one.
        unsafe { asm!("msr fpcr, {v}", v = in(reg) v, options(nomem, nostack, preserves_flags)) }
    }
    pub fn enable() -> usize {
        let old = current();
        set(old as u64 | FZ);
        old
    }
    pub fn restore(saved: usize) {
        set(saved as u64);
    }
    pub fn is_on(state: usize) -> bool {
        state as u64 & FZ != 0
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod arch {
    pub fn enable() -> usize {
        0
    }
    pub fn restore(_: usize) {}
    pub fn current() -> usize {
        0
    }
    pub fn is_on(_: usize) -> bool {
        false
    }
}

impl FtzGuard {
    /// Turns flush-to-zero on for this thread until the guard drops.
    pub fn new() -> Self {
        Self {
            saved: arch::enable(),
            _not_send: std::marker::PhantomData,
        }
    }

    /// True while flush-to-zero is active on the calling thread (always false on architectures
    /// without support).
    pub fn active() -> bool {
        arch::is_on(arch::current())
    }
}

impl Default for FtzGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FtzGuard {
    fn drop(&mut self) {
        arch::restore(self.saved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[inline(never)]
    fn mul(a: f32, b: f32) -> f32 {
        std::hint::black_box(a) * std::hint::black_box(b)
    }

    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn flushes_and_restores() {
        let before = arch::current();
        let tiny = std::hint::black_box(f32::MIN_POSITIVE);
        let half = std::hint::black_box(0.5f32);
        assert!(!FtzGuard::active());
        assert!(mul(tiny, half) > 0.0, "subnormal survives by default");
        {
            let _g = FtzGuard::new();
            assert!(FtzGuard::active());
            assert_eq!(mul(tiny, half), 0.0, "subnormal result flushed");
            {
                let _inner = FtzGuard::new();
            }
            assert!(FtzGuard::active(), "nested guard restores the outer state");
        }
        // Low MXCSR/FPCR bits are sticky exception flags that the arithmetic above sets.
        assert_eq!(
            arch::current() & !0x3f,
            before & !0x3f,
            "mode bits restored"
        );
    }
}
