//! `quantizer`: snaps a pitch to the nearest note of a scale. Pitch is semitones from C4 (as
//! `midi.in` and `seq` give it); `root` shifts the scale's tonic (0 = C), `transpose` moves the
//! result by whole semitones after snapping, so a sequence can be held to a scale and then
//! shifted without leaving it.
//!
//! `trig` is a 5 ms pulse each time the snapped note changes (an envelope trigger for a random
//! melody). The first note after the module starts does not pulse. A small hysteresis (0.05
//! semitone past the halfway point) keeps a slowly moving input, such as an LFO, from chattering
//! between two notes. Follows its input: one instance per voice behind a `midi.in`, otherwise one.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Pitch,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "trig",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
];

pub const SCALE_LABELS: [&str; 12] = [
    "CHROM", "MAJOR", "MINOR", "HARM", "DORIAN", "PHRYG", "LYDIAN", "MIXO", "PENT", "M PENT",
    "BLUES", "WHOLE",
];

const fn mask(degrees: &[u8]) -> u16 {
    let mut m = 0;
    let mut i = 0;
    while i < degrees.len() {
        m |= 1 << degrees[i];
        i += 1;
    }
    m
}

/// Pitch classes of each scale above its root, bit k = k semitones.
const SCALES: [u16; 12] = [
    0xFFF,
    mask(&[0, 2, 4, 5, 7, 9, 11]),
    mask(&[0, 2, 3, 5, 7, 8, 10]),
    mask(&[0, 2, 3, 5, 7, 8, 11]),
    mask(&[0, 2, 3, 5, 7, 9, 10]),
    mask(&[0, 1, 3, 5, 7, 8, 10]),
    mask(&[0, 2, 4, 6, 7, 9, 11]),
    mask(&[0, 2, 4, 5, 7, 9, 10]),
    mask(&[0, 2, 4, 7, 9]),
    mask(&[0, 3, 5, 7, 10]),
    mask(&[0, 3, 5, 6, 7, 10]),
    mask(&[0, 2, 4, 6, 8, 10]),
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "scale",
        min: 0.0,
        max: 11.0,
        default: 1.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "root",
        min: 0.0,
        max: 11.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "transpose",
        min: -24.0,
        max: 24.0,
        default: 0.0,
        unit: "st",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static QUANTIZER_INFO: ModuleInfo = ModuleInfo {
    kind: "quantizer",
    name: "Quantizer",
    category: Category::Utility,
    rate: Rate::Voice,
    explain:
        "Snaps a pitch to the nearest note of a scale, so random or sliding values stay in key.",
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
    advanced: &["transpose"],
};

const TRIG_S: f32 = 0.005;
const HYSTERESIS: f32 = 0.1;

pub struct Quantizer {
    sample_rate: f32,
    /// The snapped note before transposing, None before the first input.
    held: Option<f32>,
    key: (u8, u8),
    last_in: f32,
    trig_left: u32,
}

impl Quantizer {
    pub fn new() -> Self {
        Quantizer {
            sample_rate: 48000.0,
            held: None,
            key: (255, 255),
            last_in: f32::NAN,
            trig_left: 0,
        }
    }
}

impl Default for Quantizer {
    fn default() -> Self {
        Self::new()
    }
}

/// The note of `scale` (rooted at `root`) nearest to `x`; ties go down.
pub fn snap(x: f32, scale: usize, root: f32) -> f32 {
    let mask = SCALES[scale.min(11)];
    let rel = x - root;
    let octave = (rel / 12.0).floor();
    let pc = rel - octave * 12.0;
    // Allowed degrees 0..=12 (12 is the next octave's root): the nearest at or below the input and
    // the nearest at or above it, by bit scans.
    let m = mask as u32 | 1 << 12;
    let fl = (pc.floor().max(0.0) as u32).min(12);
    let lo = 31 - (m & ((2u32 << fl) - 1)).leading_zeros();
    let ce = if pc > fl as f32 { (fl + 1).min(12) } else { fl };
    let hi = (m & (u32::MAX << ce)).trailing_zeros();
    let best = if pc - lo as f32 <= hi as f32 - pc {
        lo
    } else {
        hi
    } as f32;
    root + octave * 12.0 + best
}

impl Module for Quantizer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &QUANTIZER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let input = io.input(0);
        let scale = io.param(0).at(0).round().clamp(0.0, 11.0) as u8;
        let root = io.param(1).at(0).round().clamp(0.0, 11.0) as u8;
        let transpose = io.param(2).at(0).round().clamp(-24.0, 24.0);
        let trig_len = (TRIG_S * self.sample_rate) as u32;
        let key_changed = self.key != (scale, root);
        self.key = (scale, root);
        let mut fresh = key_changed;
        for i in 0..n {
            let x = input.at(i);
            if x.is_finite() && (x != self.last_in || fresh) {
                self.last_in = x;
                let near = snap(x, scale as usize, root as f32);
                match self.held {
                    None => self.held = Some(near),
                    Some(h) if near == h => {}
                    // Stay on the held note until the input is clearly closer to the new one.
                    Some(h) if !fresh && (x - h).abs() - (x - near).abs() < HYSTERESIS => {}
                    Some(_) => {
                        self.held = Some(near);
                        self.trig_left = trig_len;
                    }
                }
                fresh = false;
            }
            let q = self.held.unwrap_or(0.0);
            io.output(0)[i] = q + transpose;
            io.output(1)[i] = if self.trig_left > 0 {
                self.trig_left -= 1;
                1.0
            } else {
                0.0
            };
        }
    }

    fn reset(&mut self) {
        *self = Quantizer {
            sample_rate: self.sample_rate,
            ..Quantizer::new()
        };
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        if let Some(h) = self.held {
            out.write_f32("held", h);
        }
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        self.held = s.read_f32("held").filter(|h| h.is_finite());
    }
}
