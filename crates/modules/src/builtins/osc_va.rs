//! `osc.va` (brief section 8's v1 built-in table: "saw/square/tri/sine, PolyBLEP, hard sync
//! input"). All four waveforms plus hard sync — see `dsp::FullOsc`'s doc comment for what's
//! PolyBLEP-corrected (saw, square) vs. naive (triangle — needs a distinct "polyBLAMP" correction
//! not implemented here, documented not hidden).
//!
//! Waveform selection is a discrete choice represented as a stepped float param (`0..=3`,
//! rounded in `process`) — same convention `lfo.rs` uses, and the same reasoning: `ParamInfo`
//! only models float ranges, no enum-param variant exists yet. Parameter index order mirrors
//! `lfo.rs`'s (`Sine, Triangle, Saw, Square[, S&H for lfo only]`) for consistency across the two
//! oscillator-shaped modules; the default is kept at `Saw` (index 2) specifically to match this
//! module's pre-existing saw-only behavior, not because saw is otherwise privileged.
//!
//! `sync` is an audio-rate input, edge-detected per sample (rising through the same >0.5 "gate"
//! threshold `env.adsr`/`midi.in` use) — a discrete hard-sync event needs per-sample precision,
//! not a once-per-block read like the other params.

use crate::dsp::{FullOsc, OscWaveform};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};
use crate::skin::{ControlKind, ControlSkin, ModuleSkin};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "sync",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "base_hz",
        min: 20.0,
        max: 20000.0,
        default: 261.63, // C4 — the frequency a "pitch" input of 0 semitones resolves to.
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 5.0,
    },
    ParamInfo {
        name: "waveform",
        min: 0.0,
        max: 3.0,
        default: 2.0, // Saw — matches this module's behavior before waveform selection existed.
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static OSC_VA_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.va",
    name: "VA Oscillator",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "A virtual-analog oscillator: turns a pitch into sine, triangle, saw, or square.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: true, // PolyBLEP -- saw/square only, see dsp::FullOsc's doc.
        interpolation: false,
    },
    skin: Some(&OSC_VA_SKIN),
};

static OSC_VA_PANEL_PNG: &[u8] = include_bytes!("../../assets/osc_va_panel.png");

static OSC_VA_CONTROLS: &[ControlSkin] = &[
    ControlSkin {
        id: "pitch",
        kind: ControlKind::Jack,
        pos: (0.15, 0.35),
    },
    ControlSkin {
        id: "sync",
        kind: ControlKind::Jack,
        pos: (0.15, 0.55),
    },
    ControlSkin {
        id: "out",
        kind: ControlKind::Jack,
        pos: (0.85, 0.45),
    },
    ControlSkin {
        id: "base_hz",
        kind: ControlKind::Knob,
        pos: (0.30, 0.80),
    },
    ControlSkin {
        id: "waveform",
        kind: ControlKind::Knob,
        pos: (0.70, 0.80),
    },
];

/// Demo skin proving the "custom modules can use their own background image and place their own
/// jacks/knobs" mechanism end-to-end (owner ask, not a brief feature — see decisions.md "Module
/// skins: custom panel art"). The art itself is an honest, generated placeholder, not designed
/// hardware-panel art — flagged as such, not passed off as more than it is.
static OSC_VA_SKIN: ModuleSkin = ModuleSkin {
    panel_size: (200.0, 220.0),
    background_image: Some(OSC_VA_PANEL_PNG),
    controls: OSC_VA_CONTROLS,
};

/// Input port index for `pitch` — matches `PORTS` above. Named constants because `ProcessIo`'s
/// indices are position-based (brief section 8: "indices ... match the module's `ModuleInfo`
/// order"), and a bare `0`/`1` at every call site would be easy to get wrong once modules have
/// more than one or two ports.
const PITCH_IN: usize = 0;
const SYNC_IN: usize = 1;
const OUT: usize = 0;
const BASE_HZ_PARAM: usize = 0;
const WAVEFORM_PARAM: usize = 1;

fn waveform_from_param(v: f32) -> OscWaveform {
    match v.round() as i32 {
        1 => OscWaveform::Triangle,
        2 => OscWaveform::Saw,
        3 => OscWaveform::Square,
        _ => OscWaveform::Sine,
    }
}

pub struct OscVa {
    osc: FullOsc,
    sample_rate: f32,
    /// Edge-detect state for the `sync` input, carried across recompile like `env.adsr`'s
    /// `gate_was_high` — without it, a sync line already held high at recompile time would
    /// (harmlessly, since sync's edge already fired) just miss a spurious re-trigger, but a
    /// *held-low-then-still-low* line depends on this too for correctness if sync's polarity
    /// convention ever changes. Carried on principle, matching the lesson from the env.adsr bug
    /// this same session (see docs/decisions.md): any "was this true last sample" flag a module
    /// keeps needs to be in `save_state`/`load_state`, not just its continuous values.
    sync_was_high: bool,
}

impl OscVa {
    pub fn new() -> Self {
        OscVa {
            osc: FullOsc::new(),
            sample_rate: 48000.0,
            sync_was_high: false,
        }
    }
}

impl Default for OscVa {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for OscVa {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_VA_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let sync = io.input(SYNC_IN);
        let base_hz = io.param(BASE_HZ_PARAM);
        let waveform = waveform_from_param(io.param(WAVEFORM_PARAM).at(0));
        let block_len = io.block_len();
        let sample_rate = self.sample_rate;

        let osc = &mut self.osc;
        let mut sync_was_high = self.sync_was_high;
        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            let sync_high = sync.at(i) > 0.5;
            if sync_high && !sync_was_high {
                osc.hard_sync();
            }
            sync_was_high = sync_high;

            let semitones = pitch.at(i);
            let freq = base_hz.at(i) * 2f32.powf(semitones / 12.0);
            *sample = osc.next(freq, sample_rate, waveform);
        }
        self.sync_was_high = sync_was_high;
    }

    fn reset(&mut self) {
        self.osc = FullOsc::new();
        self.sync_was_high = false;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.osc.phase);
        out.write_f32("sync_was_high", self.sync_was_high as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.osc.phase = phase;
        }
        if let Some(v) = s.read_f32("sync_was_high") {
            self.sync_was_high = v != 0.0;
        }
    }
}
