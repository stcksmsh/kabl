//! Built-in modules + `ModuleInfo` metadata (brief section 8). `dsp` holds the reusable signal
//! processing primitives (oscillators, filters, envelopes) — relocated here from `engine` once
//! this crate stopped being a stub; see docs/decisions.md 2026-09-21 "Module registry: dsp
//! relocated from engine to modules".

pub mod builtins;
pub mod dsp;
pub mod info;
pub mod io;
pub mod module;
pub mod registry;

pub use info::{
    Category, LessonId, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport,
    Rate, Taper,
};
pub use io::{ProcessIo, Signal};
pub use module::{Module, QualityConfig, QualityTier, StateReader, StateWriter};
