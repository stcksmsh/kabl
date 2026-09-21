//! Graph compiler, scheduler, voice allocator, swap/crossfade, quality tiers (brief section 7).
//! v1 milestone in progress; spike S1 (graph swap) lives here first as the de-risking work
//! ahead of the general compiler.

pub mod compile;
pub mod dyn_dispatch_spike;
pub mod graph;
pub mod patch_demo;
pub mod patch_engine;
pub mod potato;
pub mod simd_voices;
pub mod swap;
