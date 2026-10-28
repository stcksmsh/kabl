//! `split` and `join`: stereo wiring, no DSP. The compiler aliases their output buffers to their
//! input buffers (no instance, no step, no CPU); `process` only runs if a host instantiates one
//! directly, and does the same copy.
//!
//! `split` takes a stereo jack and gives its `left` and `right` channels as separate mono ports.
//! `join` takes two mono ports and gives one stereo jack; an unwired side is silence.
//!
//! Rates: `join` is global, like every stereo module it can feed, so a voice source is summed into
//! it exactly as it was summed into the old half port. `split` follows its source.

use crate::info::{Category, ModuleInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const fn port(name: &'static str, input: bool, width: u8) -> PortInfo {
    PortInfo {
        name,
        port_type: PortType::Audio,
        direction: if input {
            PortDirection::Input
        } else {
            PortDirection::Output
        },
        width,
    }
}

const SPLIT_PORTS: &[PortInfo] = &[
    port("in", true, 2),
    port("left", false, 1),
    port("right", false, 1),
];
const JOIN_PORTS: &[PortInfo] = &[
    port("left", true, 1),
    port("right", true, 1),
    port("out", false, 2),
];

const QUALITY: QualitySupport = QualitySupport {
    oversampling: false,
    anti_aliasing: false,
    interpolation: false,
};

pub static SPLIT_INFO: ModuleInfo = ModuleInfo {
    kind: "split",
    name: "Split",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Takes a stereo cable apart: left and right come out as two separate signals.",
    lesson: None,
    requires: &[],
    ports: SPLIT_PORTS,
    params: &[],
    quality: QUALITY,
    skin: None,
    width_units: 3,
    advanced: &[],
};

pub static JOIN_INFO: ModuleInfo = ModuleInfo {
    kind: "join",
    name: "Join",
    category: Category::Utility,
    rate: Rate::Global,
    explain: "Puts two signals on one stereo cable: left and right in, one stereo cable out. A side with no cable is silent.",
    lesson: None,
    requires: &[],
    ports: JOIN_PORTS,
    params: &[],
    quality: QUALITY,
    skin: None,
    width_units: 3,
    advanced: &[],
};

macro_rules! wiring {
    ($name:ident, $info:ident) => {
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                $name
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Module for $name {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn info(&self) -> &'static ModuleInfo {
                &$info
            }
            fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}
            fn process(&mut self, io: &mut ProcessIo) {
                let n = io.block_len();
                for ch in 0..2 {
                    let input = io.input(ch);
                    for (i, o) in io.output(ch)[..n].iter_mut().enumerate() {
                        *o = input.at(i);
                    }
                }
            }
            fn reset(&mut self) {}
        }
    };
}

wiring!(Split, SPLIT_INFO);
wiring!(Join, JOIN_INFO);
