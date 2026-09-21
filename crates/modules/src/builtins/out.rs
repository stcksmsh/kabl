//! `out` (brief section 8: "stereo output"). The graph's terminal sink — it has no `Module`
//! output ports of its own, because nothing downstream in the `Module` graph consumes its
//! signal; a real audio callback (`crates/standalone`, not built yet) will read `left()`/
//! `right()` each block and hand them to the device. `prepare` allocates the internal buffers
//! once (RT rule: `process` never allocates, brief section 3) — writing into already-sized
//! buffers in `process` is a plain memory write, not a reallocation.

use crate::info::{Category, ModuleInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "left",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "right",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
];

pub static OUT_INFO: ModuleInfo = ModuleInfo {
    kind: "out",
    name: "Output",
    category: Category::Utility,
    rate: Rate::Global,
    explain: "The patch's stereo output — what actually reaches your speakers or headphones.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: &[],
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
};

const LEFT: usize = 0;
const RIGHT: usize = 1;

pub struct Out {
    left: Vec<f32>,
    right: Vec<f32>,
}

impl Out {
    pub fn new() -> Self {
        Out {
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    pub fn left(&self) -> &[f32] {
        &self.left
    }

    pub fn right(&self) -> &[f32] {
        &self.right
    }
}

impl Default for Out {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Out {
    fn info(&self) -> &'static ModuleInfo {
        &OUT_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        self.left = vec![0.0; max_block];
        self.right = vec![0.0; max_block];
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let left_in = io.input(LEFT);
        let right_in = io.input(RIGHT);
        let block_len = io.block_len();
        for i in 0..block_len {
            self.left[i] = left_in.at(i);
            self.right[i] = right_in.at(i);
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
    }
}
