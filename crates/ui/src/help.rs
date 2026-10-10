//! Contextual help: short authored text per module, parameter and jack, keyed by kind and name
//! (never by patch instance or by a pin's label), plus range/unit/choice text built from
//! `ParamInfo` and the rack's own formatters. A module's description is its
//! `ModuleInfo.explain`; `module_note` adds what that one line leaves out.
//!
//! Nothing here reads the patch: `explain.rs` combines this with the current graph.

use kabl_modules::builtins::seq;
use kabl_modules::{registry, ParamInfo, Taper};

use crate::routing;

/// What a parameter does, in one or two sentences. `None` only for a kind/param this table
/// does not know (a future module): callers then show the range text alone.
pub fn param_help(kind: &str, param: &str) -> Option<&'static str> {
    let param = if kind == "seq" {
        seq::slot_name(param)
    } else {
        param
    };
    Some(match (kind, param) {
        ("osc.va", "base_hz") => {
            "Tuning: the frequency played at pitch 0 (C4 = 261.63 Hz). With a keyboard or \
             sequencer on the pitch jack, every note is shifted by the same ratio."
        }
        ("osc.va", "waveform") => {
            "Wave shape: sine is pure, triangle soft, saw bright and buzzy, square hollow."
        }
        ("osc.va", "pw") => {
            "Pulse width of the square wave: 50 % is hollow, narrower is thinner and nasal. \
             Only the square uses it."
        }
        ("osc.va", "fine") => "Fine tuning in cents (100 cents = one semitone).",
        ("osc.va", "unison") => {
            "How many oscillators play each note (1–4). More sounds thicker; the level is \
             kept the same."
        }
        ("osc.va", "detune") => {
            "Spread of the unison oscillators, lowest to highest, in cents. Needs Unison 2 or \
             more."
        }
        ("osc.fm", "base_hz") => {
            "Tuning: the frequency played at pitch 0 (C4 = 261.63 Hz), before Ratio and Fine."
        }
        ("osc.fm", "ratio") => {
            "Frequency multiple of this operator: x1 plays the note itself, x2 an octave up. \
             Whole ratios sound pitched; Fine moves off them for bells and metal."
        }
        ("osc.fm", "fine") => {
            "Detunes Ratio in cents (up to half an octave either way): x3 minus 231 cents is \
             a ratio of 2.62, a bell."
        }
        ("osc.fm", "level") => {
            "Output level. On an operator that feeds another, this sets how bright the other \
             sounds; patch an envelope here to make the brightness fade."
        }
        ("osc.fm", "index") => {
            "How far the signal on the pm jack bends this operator's phase: more is brighter and \
             more complex. Needs a cable on pm."
        }
        ("osc.fm", "feedback") => {
            "Feeds the operator back into itself: from a pure sine toward a bright saw-like buzz."
        }
        ("osc.fm" | "osc.fm6", "oversample") => {
            "Internal rate: 2X is the default; 4X costs about twice as much and keeps very bright \
             sounds cleaner."
        }
        ("osc.fm6", "base_hz") => {
            "Tuning: the frequency played at pitch 0 (C4 = 261.63 Hz), before each Ratio and Fine."
        }
        ("osc.fm6", "algorithm") => {
            "Who modulates whom: 6>5>4>3>2>1 is one long chain, ADD plays all six side by side. \
             Operators with a higher number modulate lower ones; operator 1 is always heard."
        }
        ("osc.fm6", "feedback") => {
            "Feeds operator 6 back into itself: from a pure sine toward a bright saw-like buzz."
        }
        ("osc.fm6", "index") => {
            "How strongly modulators bend the operators they feed: more is brighter and more \
             complex."
        }
        ("osc.fm6", "fine") => "Detunes the whole voice in cents (100 cents = one semitone).",
        ("osc.fm6", p) if p.starts_with("ratio") => {
            "Frequency multiple of this operator: x1 plays the note itself, x2 an octave up."
        }
        ("osc.fm6", p) if p.starts_with("fine") => {
            "Detunes this operator's Ratio in cents (up to half an octave either way)."
        }
        ("osc.fm6", p) if p.starts_with("level") => {
            "Output level. On an operator that feeds another, this sets how bright the other \
             sounds."
        }
        ("osc.fm6", p) if p.starts_with("attack") => {
            "Time this operator takes to reach full level."
        }
        ("osc.fm6", p) if p.starts_with("decay") => "Time this operator takes to fall to Sustain.",
        ("osc.fm6", p) if p.starts_with("sustain") => {
            "Level this operator holds while the key is down."
        }
        ("osc.fm6", p) if p.starts_with("release") => {
            "Time this operator takes to fade after the key is released."
        }
        ("osc.fm6", p) if p.starts_with("vel") => {
            "How much key velocity scales this operator: 0 ignores it, 1 follows it fully."
        }
        ("osc.wt", "base_hz") => {
            "Tuning: the frequency played at pitch 0 (C4 = 261.63 Hz). With a keyboard or \
             sequencer on the pitch jack, every note is shifted by the same ratio."
        }
        ("osc.wt", "table") => "Which built-in wavetable to play. Ignored while User is on.",
        ("osc.wt", "position") => {
            "Where in the table to read: the table is a row of waveforms, and this slides \
             smoothly from the first to the last. Modulate it to make the sound move."
        }
        ("osc.wt", "pos_mod") => {
            "How far a cable on the pos jack moves Position (1 = whole table)."
        }
        ("osc.wt", "fine") => "Fine tuning in cents (100 cents = one semitone).",
        ("osc.wt", "user") => {
            "OFF plays the built-in Table. 1–8 plays the wavetable saved in that slot of this \
             sound; it travels with the sound."
        }
        ("filter.svf", "cutoff_hz") | ("filter.ladder", "cutoff_hz") => {
            "Cutoff: where the filter starts removing frequencies. Lower is darker and softer, \
             higher is brighter."
        }
        ("filter.svf", "resonance") => {
            "Resonance: boosts frequencies right at the cutoff, from smooth to a whistling peak."
        }
        ("filter.ladder", "resonance") => {
            "Resonance: boosts frequencies at the cutoff. High settings sing; above about 91 % \
             the filter whistles on its own (self-oscillates)."
        }
        ("filter.ladder", "drive_db") => {
            "Input drive: pushes the filter harder, rounding and thickening the sound."
        }
        ("env.adsr", "attack_ms") => {
            "Attack: how long the envelope takes to rise when a note starts. Short plucks, \
             long swells."
        }
        ("env.adsr", "decay_ms") => "Decay: how long it takes to fall from the peak to sustain.",
        ("env.adsr", "sustain") => {
            "Sustain: the level held while the key (gate) stays down, 0–1 of the peak."
        }
        ("env.adsr", "release_ms") => {
            "Release: how long the envelope takes to fade after the key is let go."
        }
        ("env.adsr", "timing") => {
            "CONT: time changes apply at once, even to held notes. KEY: each note keeps the \
             times it started with."
        }
        ("lfo", "rate_hz") => "Speed of the wave, in cycles per second (when not synced).",
        ("lfo", "waveform") => {
            "Wave shape: smooth sine/triangle sweeps, saw ramps, square jumps, S&H random steps."
        }
        ("lfo", "sync") => {
            "FREE uses Rate. A note length makes one cycle last that long, following the clock \
             patched to the clock jack."
        }
        ("lfo", "phase") => "Where in its cycle the wave starts after a reset or sync.",
        ("vca", "gain") => {
            "Level with nothing on the CV jack. A CV (an envelope) adds to it; the total is \
             held between 0 and 1."
        }
        ("vca", "exponential") => {
            "LIN follows the CV directly; EXP squares it, so fades sound more natural."
        }
        ("mixer", p) if p.starts_with("level") => "Level of this channel into the mix, 0–1.",
        ("clock", "bpm") => "Tempo in beats per minute. The clock ticks four times per beat.",
        ("quantizer", "scale") => {
            "The notes that are allowed. Major and Minor are the everyday scales; Pentatonic and \
             Blues have fewer notes and are hard to play wrong; Chromatic allows every note."
        }
        ("quantizer", "root") => {
            "The scale's home note, in semitones above C: 0 is C, 2 is D, 9 is A."
        }
        ("quantizer", "transpose") => {
            "Moves the result up or down in whole semitones, after the pitch has been snapped."
        }
        ("sample.hold", "mode") => {
            "Sample keeps the value from the moment of each tick. Track follows the signal while \
             the clock is high and holds the last value when it goes low."
        }
        ("slew", "rise_ms") => {
            "How long the output takes to climb to a higher value. 0.1 ms passes the signal \
             as it is."
        }
        ("slew", "fall_ms") => {
            "How long the output takes to drop to a lower value. 0.1 ms passes the signal as it is."
        }
        ("slew", "mode") => {
            "Slew smooths the signal. Follow first turns it into its size (ignoring up and \
             down), so Rise and Fall become the attack and release of a loudness follower."
        }
        ("attenuverter", "amount") => {
            "Multiplies the signal: 1 leaves it, under 1 makes it smaller, negative flips it \
             upside down, up to 24 times bigger."
        }
        ("attenuverter", "offset") => "Added after scaling: moves the whole signal up or down.",
        ("comparator", "threshold") => "The level the signal has to pass for the gate to go high.",
        ("comparator", "hysteresis") => {
            "A dead band around the threshold. Wider stops a slow or noisy signal from \
             flickering the gate on and off."
        }
        ("crossfade", "mix") => {
            "The blend: 0 is all of A, 1 is all of B. The fade input adds to it."
        }
        ("crossfade", "curve") => {
            "0 is exact for control signals. 1 keeps two different sounds at an even loudness \
             through the middle."
        }
        ("pan", "pan") => {
            "Left to right: -1 is all left, 0 centre, 1 all right. The pan input adds to it."
        }
        ("random", "length") => {
            "0 gives a new value every tick, never repeating. 1 to 16 makes a loop of that many \
             values that comes round again."
        }
        ("random", "change") => {
            "In a loop, the chance that each value is replaced by a new one as it comes round: \
             0 keeps the loop exactly, higher lets it drift."
        }
        ("random", "bipolar") => "Uni gives values from 0 up; Bi gives values on both sides of 0.",
        ("random", "range") => {
            "How big the values are: 1 gives up to 1; for pitch use 12 or more, so the values \
             span octaves."
        }
        ("clock.div", "div") => "Passes every Nth tick: 2 halves the speed, 4 quarters it.",
        ("seq", s) if s.len() == 2 && s.starts_with('p') => {
            "Pitch of this step in semitones from the reference note."
        }
        ("seq", s) if s.len() == 2 && s.starts_with('g') => {
            "Step on or off. An off step is a rest."
        }
        ("seq", s) if s.len() == 2 && s.starts_with('v') => {
            "Velocity (strength) of this step; patch the velocity jack to use it."
        }
        ("seq", s) if s.len() == 2 && s.starts_with('r') => {
            "Chance that this step plays each time round; the rest of the time it is a rest."
        }
        ("seq", "length") => "How many steps the pattern plays before it loops (1–8).",
        ("seq", "transpose") => "Shifts every step by whole semitones.",
        ("seq", "gate_len") => "In LENGTH gate mode, how long each note is held, as % of a step.",
        ("seq", "gate_mode") => {
            "CLOCK: notes follow the clock pulse. LENGTH: notes are held for Gate length."
        }
        ("seq", "direction") => "Play order: forward, reverse, or back and forth.",
        ("seq", "bank") => "The bank (A–D) the sequencer plays when the patch opens.",
        ("delay", "time_ms") => "Time between echoes when Sync is FREE.",
        ("delay", "sync") => {
            "FREE uses Time. A note length follows the clock patched to the clock jack."
        }
        ("delay", "feedback") => "How much of each echo repeats again: more repeats, longer tail.",
        ("delay", "mix") => "Echo level against the dry sound: 0 % dry only, 100 % echoes only.",
        ("delay", "tone_hz") => "Darkens each repeat: lower is darker. The first echo stays full.",
        ("delay", "mode") => "MONO: echoes in the middle. PING: echoes bounce left and right.",
        ("reverb", "decay_s") => "How long the reverb tail lasts.",
        ("reverb", "damp_hz") => "Darkens the tail: lower sounds like a softer, bigger room.",
        ("reverb", "mix") => "Reverb level against the dry sound: 100 % is reverb only.",
        ("reverb", "predelay_ms") => "Gap before the reverb starts; keeps attacks clear.",
        ("reverb", "width") => "Stereo width of the tail: 0 % is mono.",
        ("gain", "gain_db") => "Level change in dB: 0 dB leaves it unchanged, −6 dB is about half.",
        ("macro", _) => {
            "A performance knob: it does nothing by itself. Each route from its output moves \
             one destination by that route's depth and direction."
        }
        ("noise", "color") => "WHITE is bright hiss; PINK is softer, like wind or surf.",
        ("noise", "level_db") => "Output level of the noise.",
        ("chorus", "rate_hz") => "Speed of the moving copies: slow shimmer to fast warble.",
        ("chorus", "depth") => "How far the copies move: more sounds wider and more detuned.",
        ("chorus", "mix") => "Chorus level against the dry sound.",
        ("chorus", "width") => "Stereo spread: 0 % keeps it centred.",
        ("drive", "drive_db") => "How hard the signal is pushed: more is rounder, then grittier.",
        ("drive", "trim_db") => "Level after the drive, to match the clean sound.",
        ("drive", "mix") => "Driven level against the clean sound.",
        ("midi.in", "mode") => {
            "POLY plays chords. MONO plays one note, retriggering each key. LEGATO plays one \
             note and only retriggers after a gap."
        }
        ("midi.in", "priority") => {
            "Which held key a MONO/LEGATO voice plays: the last pressed, lowest or highest."
        }
        ("midi.in", "glide") => {
            "Slides pitch between notes in MONO and LEGATO modes: OFF, ALWAYS, or LEGATO (only \
             between overlapping notes)."
        }
        ("midi.in", "glide_ms") => "How long a glide takes.",
        ("midi.in", "bend") => {
            "How far the pitch bend wheel moves this keyboard's notes, in semitones either \
             way. Center is no bend."
        }
        ("midi.in", "channel") => {
            "Which MIDI channel plays this keyboard. ALL takes every channel, so every \
             keyboard in the patch plays together; 1–16 lets two keyboards be played apart."
        }
        _ => return None,
    })
}

/// What a jack carries. `None` when the name says it all.
pub fn port_help(kind: &str, port: &str) -> Option<&'static str> {
    Some(match (kind, port) {
        ("ringmod", "a" | "b") => {
            "Multiplied with the other input. With a slow wave on one input and a knob or \
             macro on the other, it works as a depth control."
        }
        ("vca", "cv") => "Adds to Gain: an envelope here opens the VCA for each note.",
        ("vca", "in") => "The signal to be made louder or quieter.",
        ("osc.va", "pitch") => "Pitch in semitones from the Frequency setting.",
        ("osc.va", "sync") => "Restarts the wave on each rising edge (hard sync).",
        ("osc.fm" | "osc.fm6" | "osc.wt", "pitch") => {
            "Pitch in semitones from the Frequency setting."
        }
        ("osc.fm", "pm") => {
            "Phase modulation: the signal here bends this operator's phase by Index. Patch \
             another operator's out here."
        }
        ("osc.fm6", "gate") => {
            "A rising gate starts the note; falling starts every operator's release."
        }
        ("osc.fm6", "velocity") => "Key velocity, used by operators whose Vel is above 0.",
        ("osc.wt", "pos") => "Moves Position at audio rate: an LFO or envelope sweeps the table.",
        ("filter.svf" | "filter.ladder", "cutoff_cv") => {
            "Moves the cutoff exponentially: +1 is one octave up."
        }
        ("filter.svf", "resonance_cv") => "Adds to Resonance.",
        ("filter.svf", "lp") => "Low-pass: keeps lows.",
        ("filter.svf", "bp") => "Band-pass: keeps a band around the cutoff.",
        ("filter.svf", "hp") => "High-pass: keeps highs.",
        ("env.adsr", "gate") => "Starts the envelope while high, releases it when low.",
        ("lfo" | "delay", "clock") => "A clock for Sync to follow.",
        ("lfo", "reset") => "Restarts the wave at Phase.",
        ("seq", "clock") => "Each rising edge plays the next step.",
        ("seq", "reset") => "Jumps back to the first step.",
        ("quantizer", "in") => "The pitch to snap to the scale.",
        ("quantizer", "out") => "The pitch, moved onto the nearest note of the scale.",
        ("quantizer", "trig") => {
            "A short pulse each time the note changes: patch it to an envelope's gate."
        }
        ("sample.hold", "in") => "The signal to take values from: noise, an LFO, a sequence.",
        ("sample.hold", "clock") => {
            "Each rising edge takes a new value (Sample), or holds it (Track, when low)."
        }
        ("slew", "in") => "The signal to smooth.",
        ("attenuverter", "in") => "The signal to scale and shift.",
        ("logic", "a" | "b") => "A gate input: high from 0.5 up.",
        ("logic", "and") => "High while both A and B are high.",
        ("logic", "or") => "High while A or B (or both) is high.",
        ("logic", "xor") => "High while exactly one of A and B is high.",
        ("logic", "not") => "High while A is low.",
        ("comparator", "in") => "The signal to compare with the threshold.",
        ("comparator", "out") => "High while the signal is above the threshold.",
        ("crossfade", "a" | "b") => "One of the two signals to blend.",
        ("crossfade", "fade") => "Adds to Mix: an LFO or envelope here moves the blend.",
        ("pan", "in") => "The mono signal to place.",
        ("pan", "pan") => "Adds to Pan: an LFO here sweeps the sound across the stereo field.",
        ("random", "clock") => "Each rising edge picks the next value.",
        ("random", "reset") => "A rising edge returns to the start: the same values come again.",
        ("clock.div", "clock") => "The clock to divide.",
        ("mixer", p) if p.starts_with("in") => "One channel of the mix.",
        ("out", _) => "Reaches your speakers or headphones.",
        _ => return None,
    })
}

/// What a module is for, beyond `ModuleInfo.explain`: how it is used in the factory sounds.
pub fn module_note(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "macro" => {
            "A macro's own value is only a position, 0–1. What it changes is listed under its \
             destinations: every route from its output, with its own depth and direction."
        }
        "ringmod" => {
            "In several factory sounds a macro feeds one input and an LFO the other, so the \
             macro sets how much the LFO moves its destinations."
        }
        "midi.in" => {
            "Its settings apply to the voices this keyboard drives, not to other keyboards. \
             The wheel jack carries the modulation wheel (0–1): route it to any knob."
        }
        _ => return None,
    })
}

/// Perform cards that are not a single parameter: (title, help).
pub fn special_pin_help(key: &str) -> Option<&'static str> {
    Some(match key {
        crate::perform::TRANSPORT => {
            "Starts, stops and restarts this clock and every sequencer it drives. Runtime \
             only: it never changes the saved sound."
        }
        crate::perform::BANKS => {
            "Launches one of this sequencer's four patterns (banks A–D) on the next step or bar."
        }
        crate::perform::CUE_PADS => {
            "Each cue switches several sequencers to their chosen banks together."
        }
        _ => return None,
    })
}

/// `20 Hz – 20.00 kHz · exponential · default 1.00 kHz`, or the stepped choices.
pub fn range_text(kind: &str, p: &ParamInfo) -> String {
    let def = routing::fmt_value(p, p.default);
    if p.taper == Taper::Stepped {
        let n = (p.max - p.min).round() as usize + 1;
        let names: Vec<String> = match routing::step_labels(kind, p.name) {
            Some(l) => l.iter().take(n).map(|s| s.to_string()).collect(),
            None => (0..n).map(|k| format!("{}", p.min + k as f32)).collect(),
        };
        let default = routing::step_labels(kind, p.name)
            .and_then(|l| l.get((p.default - p.min).round() as usize).copied())
            .map_or(def, str::to_string);
        return format!("Choices: {} · default {default}", names.join(", "));
    }
    let taper = match p.taper {
        Taper::Exponential => "exponential",
        Taper::Cubic => "fine near the centre",
        _ => "linear",
    };
    let unit = if p.unit.is_empty() { " (no unit)" } else { "" };
    format!(
        "{} – {}{unit} · {taper} · default {def}",
        routing::fmt_value(p, p.min),
        routing::fmt_value(p, p.max)
    )
}

/// Module title with its user-facing name and id: `Ladder Filter #7`.
pub fn module_title(kind: &str, id: kabl_core::ModuleId) -> String {
    let name = registry::info_for(kind).map_or("Unknown module", |i| i.name);
    format!("{name} #{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every parameter of every built-in kind has authored help, and every module a
    /// description. A new module or param added without help fails here.
    #[test]
    fn every_builtin_param_has_help() {
        for info in registry::all_infos() {
            assert!(
                !info.explain.trim().is_empty(),
                "{} has no explain",
                info.kind
            );
            for p in info.params {
                assert!(
                    param_help(info.kind, p.name).is_some(),
                    "{}.{} has no help",
                    info.kind,
                    p.name
                );
            }
        }
    }

    #[test]
    fn range_text_uses_units_and_step_names() {
        let svf = registry::info_for("filter.svf").unwrap();
        assert_eq!(
            range_text("filter.svf", &svf.params[0]),
            "20 Hz – 20.00 kHz · exponential · default 1.00 kHz"
        );
        let osc = registry::info_for("osc.va").unwrap();
        let wave = osc.params.iter().find(|p| p.name == "waveform").unwrap();
        assert_eq!(
            range_text("osc.va", wave),
            "Choices: SIN, TRI, SAW, SQR · default SAW"
        );
        let lfo = registry::info_for("lfo").unwrap();
        let sync = lfo.params.iter().find(|p| p.name == "sync").unwrap();
        assert!(range_text("lfo", sync).starts_with("Choices: FREE, 1/16"));
    }

    #[test]
    fn sequencer_bank_params_share_their_slot_help() {
        assert_eq!(param_help("seq", "c.v2"), param_help("seq", "v2"));
        assert!(param_help("seq", "d.r8").unwrap().contains("Chance"));
    }
}
