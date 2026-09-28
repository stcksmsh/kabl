//! `midi.in` (brief section 8: "per-voice gate, pitch, velocity"). A source module with no
//! input ports: one instance per voice, played by the engine's keyboard
//! (`kabl_engine::keyboard`, rules in docs/sound-palette-batch/keyboard.md) through `play` and
//! `note_off`. Its params are the keyboard settings of the chain it drives: mode (POLY, MONO,
//! LEGATO), note priority, glide (OFF, ALWAYS, LEGATO) and glide time. The keyboard reads them
//! (`settings`); this module only carries out a glide (constant time, a straight line in
//! semitones, per sample) and an envelope restart (the gate held low for the first sample of
//! the next block). D06: the keyboard schedules each change at its sample offset inside the
//! block (`schedule`), so a note, release, bend or wheel move lands on its own sample; the
//! params stay block rate. Bend range and the channel filter are params too, and the wheel is
//! an output (docs/midi-timing/design.md, section 7).

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
    // The modulation wheel (CC 1), 0..1, the same on every voice of this keyboard.
    PortInfo {
        name: "wheel",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    // POLY, MONO (retrigger), LEGATO.
    ParamInfo {
        name: "mode",
        min: 0.0,
        max: 2.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    // LAST, LOW, HIGH.
    ParamInfo {
        name: "priority",
        min: 0.0,
        max: 2.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    // OFF, ALWAYS, LEGATO.
    ParamInfo {
        name: "glide",
        min: 0.0,
        max: 2.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "glide_ms",
        min: 5.0,
        max: 3000.0,
        default: 120.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    // Pitch bend range: a full bend moves the pitch this many semitones either way.
    ParamInfo {
        name: "bend",
        min: 0.0,
        max: 24.0,
        default: 2.0,
        unit: "±st",
        // A knob (25 / 17 options are too many for a selector); the module rounds.
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Which MIDI channel plays this keyboard: 0 = ALL, else 1..16.
    ParamInfo {
        name: "channel",
        min: 0.0,
        max: 16.0,
        default: 0.0,
        unit: "ch",
        // A knob (25 / 17 options are too many for a selector); the module rounds.
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static MIDI_IN_INFO: ModuleInfo = ModuleInfo {
    kind: "midi.in",
    name: "MIDI In",
    category: Category::Source,
    rate: Rate::Voice,
    explain:
        "Turns a played note into gate, pitch, and velocity signals the rest of the patch can use. Its Mode, Priority and Glide settings belong to this module: they apply only to the voices it drives.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 6,
    advanced: &["priority", "glide_ms", "bend", "channel"],
};

const GATE: usize = 0;
const PITCH: usize = 1;
const VELOCITY: usize = 2;
const WHEEL: usize = 3;
const BEND_PARAM: usize = 4;
const CHANNEL_PARAM: usize = 5;
/// Most changes one voice takes in one block (docs/midi-timing/design.md, section 3).
pub const MAX_CHANGES: usize = 64;

/// Keyboard settings of one `midi.in` (its params; `crates/engine/src/keyboard.rs` reads
/// them, docs/sound-palette-batch/keyboard.md has the rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeySettings {
    /// 0 POLY, 1 MONO, 2 LEGATO.
    pub mode: u8,
    /// 0 LAST, 1 LOW, 2 HIGH.
    pub priority: u8,
    /// 0 OFF, 1 ALWAYS, 2 LEGATO.
    pub glide: u8,
    /// 0 ALL, else the one MIDI channel (1..16) this keyboard plays from.
    pub channel: u8,
}

impl KeySettings {
    /// From the params' values, in `PARAMS` order.
    pub fn from_params(p: &[f32]) -> Self {
        let at = |i: usize| p.get(i).map_or(0, |v| v.round().clamp(0.0, 2.0) as u8);
        KeySettings {
            mode: at(0),
            priority: at(1),
            glide: at(2),
            channel: p
                .get(CHANNEL_PARAM)
                .map_or(0, |v| v.round().clamp(0.0, 16.0) as u8),
        }
    }
}

/// A keyboard change for one voice, at a sample offset inside the next block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// `MidiIn::play`'s arguments.
    Play {
        pitch: f32,
        velocity: f32,
        glide: bool,
        retrigger: bool,
    },
    Release,
    /// Bend -1..1 (times the bend range) and wheel 0..1.
    Expression {
        bend: f32,
        wheel: f32,
    },
}

#[derive(Debug, Clone, Copy)]
struct Core {
    gate: f32,
    /// The note's pitch, semitones from `osc.va`'s `base_hz`.
    pitch: f32,
    velocity: f32,
    /// The pitch output right now (differs from `pitch` while gliding).
    at: f32,
    /// Where the glide in progress started.
    from: f32,
    /// A pitch has been played (the first note of a voice never glides).
    played: bool,
    /// Hold the gate low for the first sample of the next block (envelope restart).
    dip: bool,
    /// The gate was high at the end of the last block: a release and a new note inside one
    /// block still restart the envelope.
    high_out: bool,
    settings: KeySettings,
    glide_ms: f32,
    /// Bend, -1..1, and wheel, 0..1, from the keyboard.
    bend: f32,
    wheel: f32,
    /// Changes for the next block, sorted by offset (equal offsets in arrival order).
    changes: [(u8, Change); MAX_CHANGES],
    n_changes: u8,
}

pub struct MidiIn {
    c: Core,
    sample_rate: f32,
}

impl MidiIn {
    pub fn new() -> Self {
        MidiIn {
            c: Core {
                gate: 0.0,
                pitch: 0.0,
                velocity: 0.0,
                at: 0.0,
                from: 0.0,
                played: false,
                dip: false,
                high_out: false,
                settings: KeySettings::default(),
                glide_ms: PARAMS[3].default,
                bend: 0.0,
                wheel: 0.0,
                changes: [(0, Change::Release); MAX_CHANGES],
                n_changes: 0,
            },
            sample_rate: 48000.0,
        }
    }

    /// The old entry point: a note that starts at its pitch, no envelope restart.
    pub fn note_on(&mut self, pitch_semitones: f32, velocity: f32) {
        self.play(pitch_semitones, velocity, false, false);
    }

    pub fn note_off(&mut self) {
        self.c.gate = 0.0;
    }

    /// Sounds `pitch` at `velocity`: glides there from the current pitch if `glide` (and a
    /// pitch was played before), and restarts the envelope with a one-sample gate dip if
    /// `retrigger` and the gate is high (or was at the end of the last block).
    pub fn play(&mut self, pitch: f32, velocity: f32, glide: bool, retrigger: bool) {
        let c = &mut self.c;
        c.dip |= retrigger && (c.gate > 0.5 || c.high_out);
        c.gate = 1.0;
        c.velocity = velocity.clamp(0.0, 1.0);
        c.pitch = pitch;
        if glide && c.played {
            c.from = c.at;
        } else {
            (c.at, c.from) = (pitch, pitch);
        }
        c.played = true;
    }

    /// Queues `change` at sample `offset` (clamped into the block) of the next `process`.
    /// Sorted insert, stable for equal offsets. Full: a release replaces the last change (a
    /// gate never sticks high), anything else is dropped. No allocation.
    pub fn schedule(&mut self, offset: usize, change: Change) {
        let c = &mut self.c;
        let offset = offset.min(u8::MAX as usize) as u8;
        let n = c.n_changes as usize;
        if n == MAX_CHANGES {
            if change == Change::Release {
                c.changes[n - 1].1 = Change::Release;
            }
            return;
        }
        let at = c.changes[..n]
            .iter()
            .rposition(|&(o, _)| o <= offset)
            .map_or(0, |i| i + 1);
        c.changes.copy_within(at..n, at + 1);
        c.changes[at] = (offset, change);
        c.n_changes += 1;
    }

    /// Bend (-1..1) and wheel (0..1) now.
    pub fn expression(&self) -> (f32, f32) {
        (self.c.bend, self.c.wheel)
    }

    /// Gate high (a note is sounding).
    pub fn gate(&self) -> bool {
        self.c.gate > 0.5
    }

    /// The settings from the params last seen (at compile, then every block).
    pub fn settings(&self) -> KeySettings {
        self.c.settings
    }

    /// Sets the settings before the first block (the compiler, from the base params).
    pub fn configure(&mut self, params: &[f32]) {
        self.c.settings = KeySettings::from_params(params);
        if let Some(&ms) = params.get(3) {
            self.c.glide_ms = ms;
        }
    }
}

impl Default for MidiIn {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MidiIn {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &MIDI_IN_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        // Hand-wired callers from before D06 pass four params and three outputs.
        let p = [0, 1, 2, 3, BEND_PARAM, CHANNEL_PARAM].map(|i| {
            if i < io.param_count() {
                io.param(i).at(0)
            } else {
                PARAMS[i].default
            }
        });
        let wheel_out = io.output_count() > WHEEL;
        let sample_rate = self.sample_rate;
        let c = &mut self.c;
        c.settings = KeySettings::from_params(&p);
        c.glide_ms = p[3].clamp(5.0, 3000.0);
        let range = p[4].round().clamp(0.0, 24.0);
        // Constant-time glide: the whole interval takes glide_ms, a straight line in semitones.
        let step_of = |c: &Core| (c.pitch - c.from).abs() / (c.glide_ms * 0.001 * sample_rate);
        let mut step = step_of(c);
        let changes = c.changes;
        let count = std::mem::take(&mut c.n_changes) as usize;
        let mut next = 0;
        // The gate of the sample before (the last block's last sample at first): a note that
        // replaces a sounding one at the same sample dips the gate for that sample.
        let mut high = c.high_out;
        for i in 0..n {
            while next < count && changes[next].0 as usize <= i {
                match changes[next].1 {
                    Change::Play {
                        pitch,
                        velocity,
                        glide,
                        retrigger,
                    } => {
                        c.dip |= retrigger && (c.gate > 0.5 || high);
                        c.gate = 1.0;
                        c.velocity = velocity.clamp(0.0, 1.0);
                        c.pitch = pitch;
                        if glide && c.played {
                            c.from = c.at;
                        } else {
                            (c.at, c.from) = (pitch, pitch);
                        }
                        c.played = true;
                        step = step_of(c);
                    }
                    Change::Release => c.gate = 0.0,
                    Change::Expression { bend, wheel } => {
                        c.bend = bend.clamp(-1.0, 1.0);
                        c.wheel = wheel.clamp(0.0, 1.0);
                    }
                }
                next += 1;
            }
            let gate = if std::mem::take(&mut c.dip) {
                0.0
            } else {
                c.gate
            };
            high = c.gate > 0.5;
            if c.at != c.pitch {
                let d = c.pitch - c.at;
                c.at = if d.abs() <= step {
                    c.pitch
                } else {
                    c.at + step.copysign(d)
                };
            }
            let bend = c.bend * range;
            io.output(GATE)[i] = gate;
            io.output(PITCH)[i] = if bend == 0.0 { c.at } else { c.at + bend };
            io.output(VELOCITY)[i] = c.velocity;
            if wheel_out {
                io.output(WHEEL)[i] = c.wheel;
            }
        }
        c.high_out = high;
    }

    fn reset(&mut self) {
        let settings = self.c.settings;
        let glide_ms = self.c.glide_ms;
        self.c = MidiIn::new().c;
        (self.c.settings, self.c.glide_ms) = (settings, glide_ms);
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("gate", self.c.gate);
        out.write_f32("pitch", self.c.pitch);
        out.write_f32("velocity", self.c.velocity);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("gate") {
            self.c.gate = v;
        }
        if let Some(v) = s.read_f32("pitch") {
            (self.c.pitch, self.c.at, self.c.from) = (v, v, v);
            self.c.played = true;
        }
        if let Some(v) = s.read_f32("velocity") {
            self.c.velocity = v;
        }
    }

    /// Everything, including a glide in progress and a pending dip. The settings come from the
    /// new graph's own params (`configure`), not the old graph's.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<MidiIn>() {
            let (settings, glide_ms) = (self.c.settings, self.c.glide_ms);
            self.c = o.c;
            (self.c.settings, self.c.glide_ms) = (settings, glide_ms);
        }
    }
}
