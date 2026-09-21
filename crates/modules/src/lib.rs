//! Built-in modules + `ModuleInfo` metadata (brief section 8). `dsp` holds the reusable signal
//! processing primitives (oscillators, filters, envelopes) — relocated here from `engine` once
//! this crate stopped being a stub; see docs/decisions.md 2026-09-21 "Module registry: dsp
//! relocated from engine to modules".

pub mod dsp;
