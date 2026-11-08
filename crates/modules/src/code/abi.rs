//! The code-module ABI (D11 decision 8). **Preview** until kabl 1.0: see `docs/sdk/ABI.md`, the
//! document the licence's module exception names, for the whole contract.
//!
//! One call per audio block, never per sample. Audio buffers live in the module's linear memory:
//! kabl copies inputs in and outputs out around each `process` call. Parameters are read from a
//! shared array in module memory that kabl writes before every block (one value per block;
//! modulation and smoothing are applied by kabl exactly as for built-in modules).
//!
//! Required exports: `init(sample_rate: f32, max_block: i32) -> i32` (address of a [`Layout`]
//! record, see [`Layout::from_words`]), `process(n_frames: i32)`, `reset()`. Optional:
//! `layout_ext`, `state_save`, `state_load`, `migrate`. No imports except kabl's own ABI: no WASI,
//! nothing else.

use std::ops::Range;

pub const ABI_VERSION: u32 = 0;
pub const EXPORT_INIT: &str = "init";
pub const EXPORT_PROCESS: &str = "process";
pub const EXPORT_RESET: &str = "reset";
pub const EXPORT_STATE_SAVE: &str = "state_save";
pub const EXPORT_STATE_LOAD: &str = "state_load";
pub const EXPORT_MIGRATE: &str = "migrate";
pub const EXPORT_LAYOUT_EXT: &str = "layout_ext";

/// Minor revision of the ABI this host understands. A module's `abi_minor N` meta line is
/// refused when N is greater (it may need something this host lacks); no line means 0.
pub const ABI_MINOR: u32 = 1;
/// Largest settings transfer buffer a module may declare (the `xfer_cap` word of `layout_ext`).
pub const MAX_XFER_CAP: usize = 64 << 10;

/// Size cap per embedded blob (decision 2).
pub const MAX_BLOB_BYTES: usize = 2 << 20;
/// Linear-memory cap per instance (decision 3). Over the cap counts as a crash.
pub const DEFAULT_MEMORY_CAP: usize = 4 << 20;

/// Where the module put its buffers, as byte offsets into its linear memory. Input and output
/// slots are `max_block` f32 each, one region per io slot (a width-2 port takes two), laid out
/// back to back; `params` is one f32 per parameter in `ModuleInfo.params` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub input: u32,
    pub output: u32,
    pub params: u32,
    pub state: u32,
    pub state_len: u32,
    pub max_block: u32,
    pub in_slots: u32,
    pub out_slots: u32,
    pub n_params: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    TooSmall { need_frames: u32, have: u32 },
    CountMismatch(&'static str),
    OutOfMemory(&'static str),
    Misaligned(&'static str),
    Overlap(&'static str, &'static str),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::TooSmall { need_frames, have } => {
                write!(
                    f,
                    "module supports blocks of {have} frames, host needs {need_frames}"
                )
            }
            LayoutError::CountMismatch(w) => write!(f, "layout {w} count differs from metadata"),
            LayoutError::OutOfMemory(w) => write!(f, "layout {w} region outside module memory"),
            LayoutError::Misaligned(w) => write!(f, "layout {w} region is not 4-byte aligned"),
            LayoutError::Overlap(a, b) => write!(f, "layout regions {a} and {b} overlap"),
        }
    }
}

impl Layout {
    /// The record `init` returns: nine little-endian u32 words in field order.
    pub fn from_words(w: [u32; 9]) -> Layout {
        Layout {
            input: w[0],
            output: w[1],
            params: w[2],
            state: w[3],
            state_len: w[4],
            max_block: w[5],
            in_slots: w[6],
            out_slots: w[7],
            n_params: w[8],
        }
    }

    /// A tightly packed layout starting at byte 0. Used by the fake backend and as the reference
    /// for module authors; real modules choose their own offsets.
    pub fn packed(
        in_slots: u32,
        out_slots: u32,
        n_params: u32,
        max_block: u32,
        state_len: u32,
    ) -> Layout {
        let slot = max_block * 4;
        let input = 0;
        let output = input + in_slots * slot;
        let params = output + out_slots * slot;
        let state = params + n_params * 4;
        Layout {
            input,
            output,
            params,
            state,
            state_len,
            max_block,
            in_slots,
            out_slots,
            n_params,
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.state as usize + self.state_len as usize
    }

    pub fn input_range(&self, slot: usize) -> Range<usize> {
        let start = self.input as usize + slot * self.max_block as usize * 4;
        start..start + self.max_block as usize * 4
    }

    pub fn output_range(&self, slot: usize) -> Range<usize> {
        let start = self.output as usize + slot * self.max_block as usize * 4;
        start..start + self.max_block as usize * 4
    }

    pub fn param_range(&self, index: usize) -> Range<usize> {
        let start = self.params as usize + index * 4;
        start..start + 4
    }

    /// Checks the layout against the module's declared ports/params and its memory size.
    pub fn validate(
        &self,
        memory_len: usize,
        in_slots: usize,
        out_slots: usize,
        n_params: usize,
        need_block: usize,
    ) -> Result<(), LayoutError> {
        if (self.max_block as usize) < need_block {
            return Err(LayoutError::TooSmall {
                need_frames: need_block as u32,
                have: self.max_block,
            });
        }
        if self.in_slots as usize != in_slots {
            return Err(LayoutError::CountMismatch("input slot"));
        }
        if self.out_slots as usize != out_slots {
            return Err(LayoutError::CountMismatch("output slot"));
        }
        if self.n_params as usize != n_params {
            return Err(LayoutError::CountMismatch("parameter"));
        }
        let slot = self.max_block as usize * 4;
        let mut regions = [
            ("input", self.input as usize, in_slots * slot),
            ("output", self.output as usize, out_slots * slot),
            ("params", self.params as usize, n_params * 4),
        ];
        for (name, start, len) in regions {
            if start % 4 != 0 {
                return Err(LayoutError::Misaligned(name));
            }
            if start.checked_add(len).is_none_or(|end| end > memory_len) {
                return Err(LayoutError::OutOfMemory(name));
            }
        }
        regions.sort_by_key(|r| r.1);
        for pair in regions.windows(2) {
            if pair[0].2 > 0 && pair[1].2 > 0 && pair[0].1 + pair[0].2 > pair[1].1 {
                return Err(LayoutError::Overlap(pair[0].0, pair[1].0));
            }
        }
        let state_end = self.state as usize + self.state_len as usize;
        if self.state_len > 0 && state_end > memory_len {
            return Err(LayoutError::OutOfMemory("state"));
        }
        Ok(())
    }
}
