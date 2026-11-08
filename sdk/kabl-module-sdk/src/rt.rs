//! Runtime behind `kabl_module!`. Not for creators: it holds all of the SDK's `unsafe`.
//! (Public so the macro can name it and so native tests can drive it.)

use crate::{Init, Io, Module, StateWriter, MAX_BLOCK, MAX_STATE_CAP};
use core::cell::UnsafeCell;
use core::mem::{size_of, MaybeUninit};
use core::ptr::{addr_of, addr_of_mut};

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
    /// The nine words `init` returns.
    layout: [u32; 9],
    /// `layout_ext` table: count, then `[xfer_addr, xfer_cap, state_version]`.
    ext: [u32; 4],
    live: bool,
    module: MaybeUninit<M>,
    arena: [u8; NA],
}

/// A static holding a [`Mem`]. The host is single threaded and the exports never re-enter, so
/// sharing it is sound.
pub struct Store<T>(UnsafeCell<MaybeUninit<T>>);

// SAFETY: see the type docs: one thread, no re-entrancy.
unsafe impl<T> Sync for Store<T> {}

impl<T> Store<T> {
    fn ptr(&self) -> *mut T {
        self.0.get().cast()
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
        Store(UnsafeCell::new(MaybeUninit::zeroed()))
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
        let p = s.ptr();
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
        let p = s.ptr();
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
        let p = s.ptr();
        // SAFETY: as in `init`.
        unsafe {
            if (*p).live {
                (*addr_of_mut!((*p).module)).assume_init_mut().reset();
            }
        }
    }

    pub fn layout_ext(s: &Store<Self>) -> i32 {
        Self::ext_table(s.ptr()) as usize as i32
    }

    /// Writes the settings into the transfer buffer; the length, or -1 on overflow or before
    /// `init`.
    pub fn state_save(s: &Store<Self>) -> i32 {
        let p = s.ptr();
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

    /// Hands `len` bytes of the transfer buffer (filled by the host) to the module: 1 if it
    /// accepted them, 0 if not.
    pub fn state_load(s: &Store<Self>, len: i32, version: i32) -> i32 {
        let p = s.ptr();
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
