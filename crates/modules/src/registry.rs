//! Module registry: `kind` string (`ModuleState.kind` in `kabl-core`'s `PatchState`, `ModuleInfo.
//! kind` here) -> a fresh `Module` instance, and the full list of known `ModuleInfo`s. This is
//! what the compiler needs to turn a patch's `AddModule { kind, .. }` ops into real modules, and
//! what a catalog UI (not built) will eventually read to show what's available.
//!
//! Was flagged as a gap in docs/STATUS.md ("no module registry/catalog struct... will matter
//! once the UI needs to enumerate everything, or the compiler needs to construct modules from
//! kind strings") — the compiler is exactly that first real need.
//!
//! A `match` over the known kinds, not a `HashMap`/`inventory`-style dynamic registration: brief
//! section 4.1's foundational decision 5 ("one module interface for built-in, composite and
//! code modules") means composites/code modules (v4+) will need a different registration story
//! anyway (they're not compiled into the binary as a fixed list) — building a fancier mechanism
//! now for a handful of statically-known kinds would be solving a problem the v4 milestone hasn't defined
//! yet. `create` returning `Option` (not panicking on an unknown kind) is deliberate: a
//! `PatchState` loaded from an old or foreign `.kabl` file naming a kind this binary doesn't
//! know is expected to happen eventually (missing pedal, older schema), not a bug to crash on.

use crate::builtins::{
    Clock, ClockDiv, Delay, Out, Reverb, Seq, CLOCK_DIV_INFO, CLOCK_INFO, DELAY_INFO, OUT_INFO,
    REVERB_INFO, SEQ_INFO,
};
use crate::builtins::{Cues, Macro, CUES_INFO, MACRO_INFO};
use crate::builtins::{
    EnvAdsr, FilterSvf, Gain, Lfo, MidiIn, Mixer, OscVa, RingMod, Vca, ENV_ADSR_INFO,
    FILTER_SVF_INFO, GAIN_INFO, LFO_INFO, MIDI_IN_INFO, MIXER_INFO, OSC_VA_INFO, RINGMOD_INFO,
    VCA_INFO,
};
use crate::module::Module;
use crate::ModuleInfo;

/// Every kind this binary can construct, matching brief section 8's v1 table.
pub const KNOWN_KINDS: &[&str] = &[
    "osc.va",
    "filter.svf",
    "env.adsr",
    "lfo",
    "vca",
    "ringmod",
    "mixer",
    "out",
    "midi.in",
    "clock",
    "seq",
    "clock.div",
    "delay",
    "reverb",
    "gain",
    "macro",
    "cues",
];

/// Builds a fresh instance of `kind`, or `None` if `kind` isn't a known built-in.
pub fn create(kind: &str) -> Option<Box<dyn Module>> {
    Some(match kind {
        "osc.va" => Box::new(OscVa::new()),
        "filter.svf" => Box::new(FilterSvf::new()),
        "env.adsr" => Box::new(EnvAdsr::new()),
        "lfo" => Box::new(Lfo::new()),
        "vca" => Box::new(Vca::new()),
        "ringmod" => Box::new(RingMod::new()),
        "mixer" => Box::new(Mixer::new()),
        "out" => Box::new(Out::new()),
        "midi.in" => Box::new(MidiIn::new()),
        "clock" => Box::new(Clock::new()),
        "seq" => Box::new(Seq::new()),
        "clock.div" => Box::new(ClockDiv::new()),
        "delay" => Box::new(Delay::new()),
        "reverb" => Box::new(Reverb::new()),
        "gain" => Box::new(Gain::new()),
        "macro" => Box::new(Macro::new()),
        "cues" => Box::new(Cues),
        _ => return None,
    })
}

/// `ModuleInfo` for every known kind, in the same order as `KNOWN_KINDS` — what a catalog UI
/// (not built) would enumerate.
static ALL_INFOS: [&ModuleInfo; 17] = [
    &OSC_VA_INFO,
    &FILTER_SVF_INFO,
    &ENV_ADSR_INFO,
    &LFO_INFO,
    &VCA_INFO,
    &RINGMOD_INFO,
    &MIXER_INFO,
    &OUT_INFO,
    &MIDI_IN_INFO,
    &CLOCK_INFO,
    &SEQ_INFO,
    &CLOCK_DIV_INFO,
    &DELAY_INFO,
    &REVERB_INFO,
    &GAIN_INFO,
    &MACRO_INFO,
    &CUES_INFO,
];

pub fn all_infos() -> &'static [&'static ModuleInfo] {
    &ALL_INFOS
}

/// Metadata for one kind, or `None` if unknown — for looking up ports/params without building an
/// instance (e.g. to validate a patch op before applying it).
pub fn info_for(kind: &str) -> Option<&'static ModuleInfo> {
    all_infos().iter().copied().find(|info| info.kind == kind)
}

/// The stored name an older patch used for `param` of `kind`, if it differs. Only the mixer's
/// channel levels were renamed (`level` -> `level1`..`level4`); an old stored `level` still sets
/// every channel, and an old route to `level` still reaches channel 1, exactly as before.
pub fn legacy_param(kind: &str, param: &str) -> Option<&'static str> {
    (kind == "mixer" && param.starts_with("level")).then_some(crate::builtins::LEGACY_LEVEL)
}
