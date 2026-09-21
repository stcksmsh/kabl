//! Built-in `Module` implementations (brief section 8's v1 table). Implemented so far: `osc.va`
//! (saw only, no hard sync — see `osc_va` module doc). The other 8 are pending.

mod osc_va;

pub use osc_va::{OscVa, OSC_VA_INFO};
