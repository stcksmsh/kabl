//! `filter.svf` (brief section 8's v1 table: "TPT/ZDF state-variable (LP/BP/HP), cutoff +
//! resonance CV"). Wraps `dsp::Svf`, exposing all three simultaneous outputs (see `dsp::
//! SvfOutputs`'s doc: no extra cost, the topology computes all three from the same recurrence).
//!
//! Cutoff and resonance are each a param (the knob's base value) plus an optional CV input port
//! that modulates it — same pattern `osc.va` used for pitch: `cutoff_cv` is 1V/oct-style
//! (exponential: `cutoff = cutoff_hz_param * 2^cv`), `resonance_cv` is linear (additive, then
//! clamped). This is a design choice, not brief-mandated (the brief just says "cutoff +
//! resonance CV"); flagged in docs/decisions.md.
//!
//! Fast path: if both CV inputs are `Signal::Scalar` for the whole block (nothing patched, or a
//! held-scalar per the compiler's control-rate tier once one exists), coefficients are computed
//! once and reused via `process_with_coeffs_multi` — exactly spike S3's finding applied for
//! real: recomputing `tan()` every sample when nothing's actually changing wastes the CPU S2
//! showed how to save. If either is a genuine per-sample buffer, falls back to `process_multi`,
//! which recomputes coefficients every sample because correctness requires it there.

use crate::dsp::{Svf, SvfCoeffs};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "cutoff_cv",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "resonance_cv",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "lp",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "bp",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "hp",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "cutoff_hz",
        min: 20.0,
        max: 20000.0,
        default: 1000.0,
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 5.0,
    },
    ParamInfo {
        name: "resonance",
        min: 0.0,
        max: 0.95,
        default: 0.3,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 5.0,
    },
];

pub static FILTER_SVF_INFO: ModuleInfo = ModuleInfo {
    kind: "filter.svf",
    name: "SVF Filter",
    category: Category::Filter,
    rate: Rate::Voice,
    explain: "Shapes a sound's tone by letting some frequencies through and blocking others.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
};

const IN: usize = 0;
const CUTOFF_CV: usize = 1;
const RESONANCE_CV: usize = 2;
const LP: usize = 0;
const BP: usize = 1;
const HP: usize = 2;
const CUTOFF_HZ_PARAM: usize = 0;
const RESONANCE_PARAM: usize = 1;

pub struct FilterSvf {
    svf: Svf,
    sample_rate: f32,
}

impl FilterSvf {
    pub fn new() -> Self {
        FilterSvf {
            svf: Svf::new(),
            sample_rate: 48000.0,
        }
    }
}

impl Default for FilterSvf {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for FilterSvf {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &FILTER_SVF_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let input = io.input(IN);
        let cutoff_cv = io.input(CUTOFF_CV);
        let resonance_cv = io.input(RESONANCE_CV);
        let cutoff_param = io.param(CUTOFF_HZ_PARAM);
        let resonance_param = io.param(RESONANCE_PARAM);
        let block_len = io.block_len();
        let sample_rate = self.sample_rate;

        if cutoff_cv.is_scalar()
            && resonance_cv.is_scalar()
            && cutoff_param.is_scalar()
            && resonance_param.is_scalar()
        {
            // Fast path: nothing modulates per-sample this block, cache the coefficients.
            let cutoff = (cutoff_param.at(0) * 2f32.powf(cutoff_cv.at(0))).clamp(20.0, 20000.0);
            let resonance = (resonance_param.at(0) + resonance_cv.at(0)).clamp(0.0, 0.95);
            let coeffs = SvfCoeffs::compute(cutoff, resonance, sample_rate);
            let svf = &mut self.svf;
            for i in 0..block_len {
                let out = svf.process_with_coeffs_multi(input.at(i), &coeffs);
                io.output(LP)[i] = out.lowpass;
                io.output(BP)[i] = out.bandpass;
                io.output(HP)[i] = out.highpass;
            }
        } else {
            let svf = &mut self.svf;
            for i in 0..block_len {
                let cutoff = (cutoff_param.at(i) * 2f32.powf(cutoff_cv.at(i))).clamp(20.0, 20000.0);
                let resonance = (resonance_param.at(i) + resonance_cv.at(i)).clamp(0.0, 0.95);
                let out = svf.process_multi(input.at(i), cutoff, resonance, sample_rate);
                io.output(LP)[i] = out.lowpass;
                io.output(BP)[i] = out.bandpass;
                io.output(HP)[i] = out.highpass;
            }
        }
    }

    fn reset(&mut self) {
        self.svf = Svf::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("ic1eq", self.svf.ic1eq);
        out.write_f32("ic2eq", self.svf.ic2eq);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("ic1eq") {
            self.svf.ic1eq = v;
        }
        if let Some(v) = s.read_f32("ic2eq") {
            self.svf.ic2eq = v;
        }
    }
}
