//! Built-in `Module` implementations (brief section 8's v1 table). All 9 named entries have a
//! `Module` impl now; two are intentionally partial — see their own doc comments:
//! - `osc.va`: saw waveform only, no hard sync.
//! - `lfo`: all 5 waveforms, no sync (brief itself defers "sync" — no clock exists yet, v3).

mod env_adsr;
mod filter_svf;
mod lfo;
mod midi_in;
mod mixer;
mod osc_va;
mod out;
mod ringmod;
mod vca;

pub use env_adsr::{EnvAdsr, ENV_ADSR_INFO};
pub use filter_svf::{FilterSvf, FILTER_SVF_INFO};
pub use lfo::{Lfo, LFO_INFO};
pub use midi_in::{MidiIn, MIDI_IN_INFO};
pub use mixer::{Mixer, MIXER_INFO};
pub use osc_va::{OscVa, OSC_VA_INFO};
pub use out::{Out, OUT_INFO};
pub use ringmod::{RingMod, RINGMOD_INFO};
pub use vca::{Vca, VCA_INFO};
