//! Runtime behind `kabl_module!`. Not for creators: it holds all of the SDK's `unsafe`.
//! (Public so the macro can name it and so native tests can drive it.)

use crate::{Init, Io, Module, StateWriter, MAX_BLOCK, MAX_STATE_CAP};
use core::cell::UnsafeCell;
use core::mem::{size_of, MaybeUninit};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
mod tests;

/// Compile-time checks for a module type; evaluated by the macro in a `const _`.
pub const fn check<M: Module>() {
    assert!(
        M::STATE_CAP <= MAX_STATE_CAP,
        "STATE_CAP is over the 64 KiB host limit"
    );
    assert!(
        M::ARENA_BYTES == 0 || cfg!(feature = "arena"),
        "ARENA_BYTES needs kabl-module-sdk's `arena` feature"
    );
}

/// The module's linear-memory image. Every field is valid when all-zero, so the whole static
/// can be zeroed (.bss) and the module value is written into `module` by `init`.
#[repr(C)]
pub struct Mem<
    M,
    const NI: usize,
    const NO: usize,
    const NP: usize,
    const NX: usize,
    const NA: usize,
> {
    input: [[f32; MAX_BLOCK]; NI],
    output: [[f32; MAX_BLOCK]; NO],
    params: [f32; NP],
    xfer: [u8; NX],
    /// A copy of the old settings while `migrate` writes the new ones into `xfer`.
    scratch: [u8; NX],
    /// The nine words `init` returns.
    layout: [u32; 9],
    /// `layout_ext` table: count, then `[xfer_addr, xfer_cap, state_version]`.
    ext: [u32; 4],
    live: bool,
    module: MaybeUninit<M>,
    arena: [u8; NA],
}

/// A static holding a [`Mem`], behind a busy flag. Every entry point takes the flag first and
/// panics (a trap on wasm) if it is already taken, so two threads, or a re-entrant call, can
/// never hold overlapping `&mut` into the module. A wasm host runs one call at a time, so the
/// flag never trips there.
pub struct Store<T>(UnsafeCell<MaybeUninit<T>>, AtomicBool);

// SAFETY: all access to the cell goes through `lock`, whose compare-exchange (acquire) and
// release store make the holder the only accessor, on any target; contention panics instead
// of waiting.
unsafe impl<T> Sync for Store<T> {}

struct Guard<'a, T>(&'a Store<T>);

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.0 .1.store(false, Ordering::Release);
    }
}

impl<T> Guard<'_, T> {
    fn ptr(&self) -> *mut T {
        self.0.ptr()
    }
}

impl<T> Store<T> {
    fn ptr(&self) -> *mut T {
        self.0.get().cast()
    }

    /// Exclusive access for the duration of one export call. Panics if another call (from
    /// another thread, or re-entrant) is in progress.
    fn lock(&self) -> Guard<'_, T> {
        let free = self
            .1
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        assert!(free, "kabl module called concurrently or re-entrantly");
        Guard(self)
    }
}

impl<
        M: Module,
        const NI: usize,
        const NO: usize,
        const NP: usize,
        const NX: usize,
        const NA: usize,
    > Mem<M, NI, NO, NP, NX, NA>
{
    /// An all-zero store (no data segment).
    pub const fn store() -> Store<Self> {
        Store(
            UnsafeCell::new(MaybeUninit::zeroed()),
            AtomicBool::new(false),
        )
    }

    fn ext_table(p: *mut Self) -> *mut [u32; 4] {
        // SAFETY: `p` points at a live, zero-valid `Mem`; only addresses are taken.
        unsafe {
            let ext = addr_of_mut!((*p).ext);
            *ext = [
                3,
                addr_of!((*p).xfer) as usize as u32,
                NX as u32,
                M::STATE_VERSION,
            ];
            ext
        }
    }

    pub fn init(s: &Store<Self>, sample_rate: f32, max_block: i32) -> i32 {
        let guard = s.lock();
        let p = guard.ptr();
        // SAFETY: single-threaded, non-reentrant; fields are accessed only through
        // projections of `p`, so no reference to the whole `Mem` ever exists and the arena
        // slices handed to the module stay disjoint from everything else.
        unsafe {
            (*p).live = false;
            #[cfg(feature = "arena")]
            let arena = crate::Bump::new(&mut *addr_of_mut!((*p).arena));
            let mut init = Init {
                sample_rate,
                max_block: (max_block.max(1) as usize).min(MAX_BLOCK),
                #[cfg(feature = "arena")]
                arena,
            };
            (*addr_of_mut!((*p).module)).write(M::new(&mut init));
            (*p).live = true;
            let lay = addr_of_mut!((*p).layout);
            *lay = [
                addr_of!((*p).input) as usize as u32,
                addr_of!((*p).output) as usize as u32,
                addr_of!((*p).params) as usize as u32,
                addr_of!((*p).module) as usize as u32,
                size_of::<M>() as u32,
                MAX_BLOCK as u32,
                NI as u32,
                NO as u32,
                NP as u32,
            ];
            Self::ext_table(p);
            lay as usize as i32
        }
    }

    pub fn process(s: &Store<Self>, n: i32) {
        let guard = s.lock();
        let p = guard.ptr();
        // SAFETY: as in `init`; input, output, params and module are disjoint fields.
        unsafe {
            if !(*p).live {
                return;
            }
            let mut io = Io {
                n: (n.max(0) as usize).min(MAX_BLOCK),
                input: &*addr_of!((*p).input),
                output: &mut *addr_of_mut!((*p).output),
                params: &*addr_of!((*p).params),
            };
            (*addr_of_mut!((*p).module))
                .assume_init_mut()
                .process(&mut io);
        }
    }

    pub fn reset(s: &Store<Self>) {
        let guard = s.lock();
        let p = guard.ptr();
        // SAFETY: as in `init`.
        unsafe {
            if (*p).live {
                (*addr_of_mut!((*p).module)).assume_init_mut().reset();
            }
        }
    }

    pub fn layout_ext(s: &Store<Self>) -> i32 {
        let guard = s.lock();
        Self::ext_table(guard.ptr()) as usize as i32
    }

    /// Writes the settings into the transfer buffer; the length, or -1 on overflow or before
    /// `init`.
    pub fn state_save(s: &Store<Self>) -> i32 {
        let guard = s.lock();
        let p = guard.ptr();
        // SAFETY: as in `init`; xfer and module are disjoint fields.
        unsafe {
            if !(*p).live {
                return -1;
            }
            let mut w = StateWriter::new(&mut *addr_of_mut!((*p).xfer));
            (*addr_of!((*p).module)).assume_init_ref().save(&mut w);
            if w.overflowed() {
                -1
            } else {
                w.len() as i32
            }
        }
    }

    /// Converts the `len` bytes of old settings the host put in the transfer buffer (written
    /// under `old_version`) in place: the new length, or -1 when the module cannot convert them.
    pub fn migrate(s: &Store<Self>, len: i32, old_version: i32) -> i32 {
        let guard = s.lock();
        let p = guard.ptr();
        if len < 0 || len as usize > NX {
            return -1;
        }
        // SAFETY: as in `init`; xfer and scratch are disjoint fields and no reference to the
        // module is taken (migration needs none).
        unsafe {
            let scratch: &mut [u8] = &mut *addr_of_mut!((*p).scratch);
            let xfer: &[u8] = &*addr_of!((*p).xfer);
            let old = &mut scratch[..len as usize];
            old.copy_from_slice(&xfer[..len as usize]);
            let mut w = StateWriter::new(&mut *addr_of_mut!((*p).xfer));
            if M::migrate(old_version as u32, old, &mut w) && !w.overflowed() {
                w.len() as i32
            } else {
                -1
            }
        }
    }

    /// Hands `len` bytes of the transfer buffer (filled by the host) to the module: 1 if it
    /// accepted them, 0 if not.
    pub fn state_load(s: &Store<Self>, len: i32, version: i32) -> i32 {
        let guard = s.lock();
        let p = guard.ptr();
        // SAFETY: as in `init`.
        unsafe {
            if !(*p).live || len < 0 || len as usize > NX {
                return 0;
            }
            let bytes: &[u8] = &*addr_of!((*p).xfer);
            let bytes = &bytes[..len as usize];
            i32::from(
                (*addr_of_mut!((*p).module))
                    .assume_init_mut()
                    .load(version as u32, bytes),
            )
        }
    }
}
