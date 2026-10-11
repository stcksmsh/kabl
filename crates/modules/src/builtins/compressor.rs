//! `compressor`: a stereo-linked feed-forward compressor with a soft knee, a sidechain input
//! and a gain-reduction output.
//!
//! **Detector.** The larger of the two sides' absolute values (so the stereo image does not
//! lean when one side is louder), or, when a cable is patched to `sc`, the same of `sc` alone
//! (the sound on `in_l`/`in_r` is then compressed by what is on `sc`). An unpatched `sc` reads
//! as a held scalar and falls back to the internal detector.
//!
//! **Gain computer.** In dB, with threshold T, ratio R, knee width W and level `x`: no
//! reduction below T − W/2; inside the knee a quadratic `(1 − 1/R)·(x − T + W/2)²/(2W)`; above
//! it `(1 − 1/R)·(x − T)`. The reduction (dB, ≥ 0) is smoothed with a one-pole of `attack_ms`
//! while it grows and `release_ms` while it falls (smoothing in the dB domain, after the gain
//! computer), then applied with `makeup_db`. Attack and release come from the shared
//! `TimeCoef`: no `exp` per sample while they hold still.
//!
//! **Outputs.** `gr` carries the smoothed gain reduction in dB (0 = none, positive = turned
//! down), for a meter or as modulation. With the threshold at 0 dB, ratio 1 and no makeup the
//! module passes the input sample for sample.
//!
//! **Modulation.** Threshold, ratio, attack, release and knee are read every sample (cables
//! take effect at audio rate; a step is absorbed by the smoothed reduction), makeup glides
//! (20 ms) so it never zippers. No lookahead: the module adds no latency.

use super::fx::{
    coef, param, port, Glide, TimeCoef, DB_PER_NEPER, NEPER_PER_DB, STEREO_IN, STEREO_OUT,
};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    STEREO_IN[0],
    STEREO_IN[1],
    port("sc", PortType::Audio, true),
    STEREO_OUT[0],
    STEREO_OUT[1],
    port("gr", PortType::Cv, false),
];

const PARAMS: &[ParamInfo] = &[
    param("threshold_db", -60.0, 0.0, -18.0, "dB", Taper::Linear),
    param("ratio", 1.0, 20.0, 4.0, ":1", Taper::Linear),
    param("attack_ms", 0.1, 200.0, 10.0, "ms", Taper::Exponential),
    param("release_ms", 5.0, 2000.0, 150.0, "ms", Taper::Exponential),
    param("knee_db", 0.0, 24.0, 6.0, "dB", Taper::Linear),
    param("makeup_db", 0.0, 24.0, 0.0, "dB", Taper::Linear),
];

pub static COMPRESSOR_INFO: ModuleInfo = ModuleInfo {
    kind: "compressor",
    name: "Compressor",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "Turns loud parts down so the sound evens out. Threshold is where it starts, ratio how hard it pushes, attack and release how fast. Patch another sound to sc to let that one do the ducking.",
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
    width_units: 8,
    advanced: &["knee_db"],
};

const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    /// Smoothed gain reduction, dB ≥ 0.
    gr: f32,
    makeup: Glide,
    attack: TimeCoef,
    release: TimeCoef,
}

pub struct Compressor {
    c: Core,
    sample_rate: f32,
}

impl Compressor {
    pub fn new() -> Self {
        Compressor {
            c: Core {
                gr: 0.0,
                makeup: Glide::default(),
                attack: TimeCoef::new(),
                release: TimeCoef::new(),
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Gain reduction in dB for input level `x_db` (≥ 0).
#[inline]
fn reduction(x_db: f32, thr: f32, ratio: f32, knee: f32) -> f32 {
    let over = x_db - thr;
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if 2.0 * over <= -knee {
        0.0
    } else if knee <= 0.01 || 2.0 * over >= knee {
        slope * over.max(0.0)
    } else {
        let t = over + 0.5 * knee;
        slope * t * t / (2.0 * knee)
    }
}

impl Module for Compressor {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &COMPRESSOR_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let (in_l, in_r, sc) = (io.input(0), io.input(1), io.input(2));
        let (thr, ratio, atk, rel, knee, makeup) = (
            io.param(0),
            io.param(1),
            io.param(2),
            io.param(3),
            io.param(4),
            io.param(5),
        );
        let external = !sc.is_scalar();
        let k = coef(GLIDE_TAU_S, sr);
        let c = &mut self.c;
        for i in 0..n {
            let (l, r) = (in_l.at(i), in_r.at(i));
            let det = if external {
                sc.at(i).abs()
            } else {
                l.abs().max(r.abs())
            };
            let x_db = DB_PER_NEPER * det.max(1e-9).ln();
            let target = reduction(
                x_db,
                thr.at(i).clamp(-60.0, 0.0),
                ratio.at(i).clamp(1.0, 20.0),
                knee.at(i).clamp(0.0, 24.0),
            );
            let a = if target > c.gr {
                c.attack.get(atk.at(i).clamp(0.1, 200.0), sr)
            } else {
                c.release.get(rel.at(i).clamp(5.0, 2000.0), sr)
            };
            c.gr = target + (c.gr - target) * a;
            if c.gr < 1e-6 {
                c.gr = 0.0;
            }
            let up = c.makeup.next(makeup, i, k).clamp(0.0, 24.0);
            let g = ((up - c.gr) * NEPER_PER_DB).exp();
            io.output(0)[i] = l * g;
            io.output(1)[i] = r * g;
            io.output(2)[i] = c.gr;
        }
    }

    fn reset(&mut self) {
        self.c = Compressor::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Compressor>() {
            if o.sample_rate == self.sample_rate {
                self.c = o.c;
            }
        }
    }
}
