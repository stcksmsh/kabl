//! Graph compiler, scheduler, voice allocator, swap/crossfade, quality tiers (brief section 7).
//! v1 milestone in progress; spike S1 (graph swap) lives here first as the de-risking work
//! ahead of the general compiler.

pub mod graph;
pub mod potato;
pub mod simd_voices;
pub mod swap;
