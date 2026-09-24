//! Built-in `Module` implementations (brief section 8's v1 table). All 9 named entries have a
//! `Module` impl now; two are intentionally partial — see their own doc comments:
//! - `osc.va`: saw waveform only, no hard sync.
//! - `lfo`: all 5 waveforms, no sync (brief itself defers "sync" — no clock exists yet, v3).

mod clock;
mod clock_div;
mod cues;
mod delay;
mod env_adsr;
mod filter_svf;
mod gain;
mod lfo;
mod macros;
mod midi_in;
mod mixer;
mod osc_va;
mod out;
mod reverb;
mod ringmod;
pub mod seq;
mod vca;

pub use clock::{Clock, Transport, CLOCK_INFO};
pub use clock_div::{ClockDiv, CLOCK_DIV_INFO};
pub use cues::{Cues, CUES, CUES_INFO};
pub use delay::{Delay, DelayLock, DELAY_INFO};
pub use env_adsr::{EnvAdsr, ENV_ADSR_INFO};
pub use filter_svf::{FilterSvf, FILTER_SVF_INFO};
pub use gain::{db_to_gain, Gain, GAIN_INFO};
pub use lfo::{Lfo, LfoSync, LFO_INFO, SYNC_LABELS, SYNC_TICKS};
pub use macros::{Macro, MACROS, MACRO_INFO};
pub use midi_in::{MidiIn, MIDI_IN_INFO};
pub use mixer::{Mixer, LEGACY_LEVEL, MIXER_INFO};
pub use osc_va::{OscVa, OSC_VA_INFO};
pub use out::{Out, OUT_INFO};
pub use reverb::{Reverb, REVERB_INFO};
pub use ringmod::{RingMod, RINGMOD_INFO};
pub use seq::{Direction, Seq, SEQ_INFO};
pub use vca::{Vca, VCA_INFO};
