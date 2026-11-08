//! `kabl-module-sdk`: write a kabl code module in safe Rust. **Preview**: the ABI and this API
//! may change, breaking, until kabl 1.0 (D12 decisions, principle 0).
//!
//! A module is a type that implements [`Module`], plus one `kabl_module!(MyType);` line. The
//! macro emits the wasm exports (`init`, `process`, `reset`, `layout_ext`, `state_save`,
//! `state_load`, `migrate`), the buffer layout and the `kabl.meta` and `kabl.notices` custom sections.
//! No `unsafe` in your crate (`#![forbid(unsafe_code)]` works), no allocator, no dependencies.
//!
//! ```ignore
//! #![no_std]
//! #![forbid(unsafe_code)]
//! use kabl_module_sdk::*;
//!
//! struct Gain;
//! impl Module for Gain {
//!     const META: Meta = Meta::new("acme.gain", 1)
//!         .name("Gain")
//!         .category(Category::Utility)
//!         .rate(Rate::Voice)
//!         .port_in(PortType::Audio, "in", 1)
//!         .port_out(PortType::Audio, "out", 1)
//!         .param("gain", "0", "4", "1", Taper::Linear, 5, "x");
//!     fn new(_: &mut Init) -> Self { Gain }
//!     fn reset(&mut self) {}
//!     fn process(&mut self, io: &mut Io) {
//!         let g = io.param(0);
//!         for i in 0..io.n { io.output[0][i] = io.input[0][i] * g; }
//!     }
//! }
//! kabl_module!(Gain);
//! ```
//!
//! # Memory
//! Every buffer is one zeroed static, so it lives in `.bss` and costs no bytes in the blob.
//! The module value is built in `new`, not as a static initialiser, for the same reason.
//!
//! # Saved settings
//! `save` writes into the transfer buffer ([`Module::STATE_CAP`] bytes, at most
//! [`MAX_STATE_CAP`]); `load` reads it back. A write past the end makes `state_save` return -1
//! and the host keeps the previous settings. `load` returning `false` makes `state_load`
//! return 0.
#![no_std]

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

#[cfg(feature = "arena")]
mod arena;
mod math;
mod meta;
pub mod rt;
mod state;

#[cfg(feature = "arena")]
pub use arena::{Bump, Zeroable};
pub use math::{exp2, flush, sin_cos};
pub use meta::{Category, Meta, PortType, Rate, Taper, ABI_MINOR, META_CAP};
pub use state::{StateReader, StateWriter};

/// Frames per io slot. A host asking for more than this is refused at load.
pub const MAX_BLOCK: usize = 2048;

/// Largest transfer buffer the host accepts (64 KiB); a larger `STATE_CAP` is a compile error.
pub const MAX_STATE_CAP: usize = 64 * 1024;

/// What `kabl.notices` carries: notices for the Rust toolchain code the compiler links into
/// every module (core, compiler_builtins with its libm, and core's Unicode tables), with the
/// licence texts copied verbatim from the toolchain (`NOTICE.txt`). Handled automatically: a
/// creator does nothing.
pub const NOTICE: &str = include_str!("../NOTICE.txt");

/// `s` as a byte array of exactly `L` bytes (used by the macro to fill the notices section).
pub const fn str_bytes<const L: usize>(s: &str) -> [u8; L] {
    let b = s.as_bytes();
    assert!(b.len() == L);
    let mut o = [0; L];
    let mut i = 0;
    while i < L {
        o[i] = b[i];
        i += 1;
    }
    o
}

/// The buffers for one block. Slots follow port declaration order, a width-2 port taking two
/// consecutive slots. Index constants are yours: write `const CUTOFF_CV: usize = 2;` and check it
/// with `const _: () = META.assert_in(CUTOFF_CV, "cutoff_cv");`.
pub struct Io<'a> {
    /// Frames in this block, at most [`MAX_BLOCK`]. Use only `[..n]` of each slot.
    pub n: usize,
    pub input: &'a [[f32; MAX_BLOCK]],
    pub output: &'a mut [[f32; MAX_BLOCK]],
    /// One value per param, declaration order. The host has already smoothed them.
    pub params: &'a [f32],
}

impl Io<'_> {
    /// Parameter `index` (declaration order).
    pub fn param(&self, index: usize) -> f32 {
        self.params[index]
    }

    /// The `n` frames of input slot `slot`.
    pub fn input(&self, slot: usize) -> &[f32] {
        &self.input[slot][..self.n]
    }

    /// The `n` frames of output slot `slot`.
    pub fn output(&mut self, slot: usize) -> &mut [f32] {
        &mut self.output[slot][..self.n]
    }
}

/// What `Module::new` is given.
pub struct Init {
    pub sample_rate: f32,
    /// The host's block size, at most [`MAX_BLOCK`].
    pub max_block: usize,
    /// Zeroed scratch of `ARENA_BYTES` (feature `arena`).
    #[cfg(feature = "arena")]
    pub arena: Bump,
}

/// A kabl code module. Implement it, then call `kabl_module!(YourType);` once.
pub trait Module: Sized + 'static {
    /// Identity, ports and params, built at compile time.
    const META: Meta;
    /// Version word saved with the settings; bump it when the saved layout changes.
    const STATE_VERSION: u32 = 1;
    /// Size in bytes of the settings transfer buffer: the most `save` may write. At most
    /// [`MAX_STATE_CAP`]. Costs no blob bytes.
    const STATE_CAP: usize = 256;
    /// Bytes of arena for `Init::arena`; needs the `arena` feature when non-zero.
    const ARENA_BYTES: usize = 0;

    fn new(init: &mut Init) -> Self;
    /// Render `io.n` frames.
    fn process(&mut self, io: &mut Io);
    /// Back to the freshly created state.
    fn reset(&mut self);
    /// Write the settings worth keeping across a save and reload. Default: none.
    fn save(&self, _w: &mut StateWriter) {}
    /// Restore settings written by `save` under `version`. Return `false` for data you do not
    /// understand; the module is left as it was. Default: accept and ignore.
    fn load(&mut self, _version: u32, _bytes: &[u8]) -> bool {
        true
    }
    /// Optional. Convert settings written under an older `STATE_VERSION` into the current
    /// layout: read `bytes`, write the result to `out`, return `true`. The host then passes the
    /// result to `load`. Return `false` when you cannot (the module starts at its defaults and
    /// the song keeps the old settings). Default: cannot migrate.
    fn migrate(_old_version: u32, _bytes: &[u8], _out: &mut StateWriter) -> bool {
        false
    }
}

/// Emits everything the host needs from the module type `$ty`: see the crate docs.
#[macro_export]
macro_rules! kabl_module {
    ($ty:ty) => {
        const _: () = {
            use $crate::{rt, Module};
            // Slot counts come from the meta builder: no hand-counted `3, 2, 3`.
            const NI: usize = <$ty as Module>::META.in_slots;
            const NO: usize = <$ty as Module>::META.out_slots;
            const NP: usize = <$ty as Module>::META.n_params;
            const NX: usize = <$ty as Module>::STATE_CAP;
            const NA: usize = <$ty as Module>::ARENA_BYTES;
            const _: () = rt::check::<$ty>();
            type M = rt::Mem<$ty, NI, NO, NP, NX, NA>;
            // All zero: .bss, not .data, so the blob does not carry the buffers.
            static STORE: rt::Store<M> = M::store();

            #[cfg_attr(target_arch = "wasm32", link_section = "kabl.meta")]
            #[used]
            static META: [u8; <$ty as Module>::META.len] = <$ty as Module>::META.bytes();
            #[cfg_attr(target_arch = "wasm32", link_section = "kabl.notices")]
            #[used]
            static NOTICES: [u8; $crate::NOTICE.len()] = $crate::str_bytes($crate::NOTICE);

            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn init(sample_rate: f32, max_block: i32) -> i32 {
                M::init(&STORE, sample_rate, max_block)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn process(n: i32) {
                M::process(&STORE, n)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn reset() {
                M::reset(&STORE)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn layout_ext() -> i32 {
                M::layout_ext(&STORE)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn state_save() -> i32 {
                M::state_save(&STORE)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn state_load(len: i32, version: i32) -> i32 {
                M::state_load(&STORE, len, version)
            }
            #[cfg_attr(target_arch = "wasm32", no_mangle)]
            #[allow(dead_code)]
            pub extern "C" fn migrate(len: i32, old_version: i32) -> i32 {
                M::migrate(&STORE, len, old_version)
            }
        };
    };
}

#[cfg(all(target_arch = "wasm32", not(test)))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // A trap: the host marks the module faulted and carries on without it.
    core::arch::wasm32::unreachable()
}
