//! A scripted `WasmEngine` for tests: no WebAssembly, just a gain module whose instances can be
//! told to trap, emit NaN, run out of fuel, go over memory or run slowly. Lets every failure
//! rule of `CodeModule` be tested without a runtime.

use super::abi::{Layout, MAX_BLOB_BYTES};
use super::engine::{CompiledModule, Fault, InstanceConfig, LoadError, WasmEngine, WasmInstance};
use super::meta::{CodeMeta, CodeParam, CodePort};
use crate::info::{Category, PortDirection, PortType, Rate, Taper};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Behavior {
    /// output = input * param 0
    Gain,
    /// `Err(Trap)` from call number `after` on.
    Trap { after: u32 },
    /// NaN in the first output sample from call number `after` on.
    NaN { after: u32 },
    /// `Err(MemoryOverCap)` from call number `after` on (a `memory.grow` past the cap).
    OverCap { after: u32 },
    /// `Err(OutOfFuel)` for `count` calls starting at call number `from`.
    OutOfFuel { from: u32, count: u32 },
    /// Sleeps `d` in every call, then behaves as `Gain`.
    Sleep(Duration),
}

/// Metadata of the fake: one audio in, one audio out, one `gain` parameter, per-voice.
pub fn fake_meta(kind: &str) -> CodeMeta {
    CodeMeta {
        kind: kind.into(),
        version: 1,
        name: "Fake gain".into(),
        explain: "Test module".into(),
        category: Category::Utility,
        rate: Rate::Voice,
        ports: vec![
            CodePort {
                name: "in".into(),
                port_type: PortType::Audio,
                direction: PortDirection::Input,
                width: 1,
            },
            CodePort {
                name: "out".into(),
                port_type: PortType::Audio,
                direction: PortDirection::Output,
                width: 1,
            },
        ],
        params: vec![CodeParam {
            name: "gain".into(),
            min: 0.0,
            max: 4.0,
            default: 1.0,
            unit: "".into(),
            taper: Taper::Linear,
            smoothing_ms: 5.0,
        }],
        advanced: vec![],
        width_units: 4,
    }
}

pub struct FakeEngine {
    pub meta: CodeMeta,
    /// Behavior per instantiation (voice 0, 1, ...); the last one repeats.
    pub behaviors: Vec<Behavior>,
    /// Extra memory bytes beyond the layout (to exercise the memory cap at load).
    pub extra_memory: usize,
}

impl FakeEngine {
    pub fn new(kind: &str, behaviors: Vec<Behavior>) -> FakeEngine {
        FakeEngine {
            meta: fake_meta(kind),
            behaviors,
            extra_memory: 0,
        }
    }
}

impl WasmEngine for FakeEngine {
    fn compile(&self, wasm: &[u8]) -> Result<Arc<dyn CompiledModule>, LoadError> {
        if wasm.len() > MAX_BLOB_BYTES {
            return Err(LoadError::TooLarge {
                bytes: wasm.len(),
                cap: MAX_BLOB_BYTES,
            });
        }
        Ok(Arc::new(FakeCompiled {
            meta: self.meta.clone(),
            behaviors: self.behaviors.clone(),
            extra_memory: self.extra_memory,
            made: AtomicUsize::new(0),
        }))
    }
}

struct FakeCompiled {
    meta: CodeMeta,
    behaviors: Vec<Behavior>,
    extra_memory: usize,
    made: AtomicUsize,
}

impl CompiledModule for FakeCompiled {
    fn meta(&self) -> &CodeMeta {
        &self.meta
    }

    fn instantiate(&self, cfg: &InstanceConfig) -> Result<Box<dyn WasmInstance>, LoadError> {
        let n = self.made.fetch_add(1, Relaxed);
        let behavior = *self
            .behaviors
            .get(n)
            .or(self.behaviors.last())
            .unwrap_or(&Behavior::Gain);
        let layout = Layout::packed(1, 1, 1, cfg.max_block, 0);
        Ok(Box::new(FakeInstance {
            layout,
            mem: vec![0; layout.memory_bytes() + self.extra_memory],
            behavior,
            calls: 0,
        }))
    }
}

struct FakeInstance {
    layout: Layout,
    mem: Vec<u8>,
    behavior: Behavior,
    calls: u32,
}

impl FakeInstance {
    fn f32_at(&self, at: usize) -> f32 {
        f32::from_le_bytes([
            self.mem[at],
            self.mem[at + 1],
            self.mem[at + 2],
            self.mem[at + 3],
        ])
    }
    fn gain(&mut self, frames: u32) {
        let g = self.f32_at(self.layout.param_range(0).start);
        let (i0, o0) = (
            self.layout.input_range(0).start,
            self.layout.output_range(0).start,
        );
        for i in 0..frames as usize {
            let v = self.f32_at(i0 + i * 4) * g;
            self.mem[o0 + i * 4..o0 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
}

impl WasmInstance for FakeInstance {
    fn layout(&self) -> Layout {
        self.layout
    }
    fn memory(&mut self) -> &mut [u8] {
        &mut self.mem
    }
    fn set_fuel(&mut self, _fuel: u64) {}

    fn call_block(&mut self, frames: u32) -> Result<u64, Fault> {
        let c = self.calls;
        self.calls += 1;
        match self.behavior {
            Behavior::Trap { after } if c >= after => return Err(Fault::Trap),
            Behavior::OverCap { after } if c >= after => return Err(Fault::MemoryOverCap),
            Behavior::OutOfFuel { from, count } if c >= from && c < from + count => {
                return Err(Fault::OutOfFuel)
            }
            Behavior::Sleep(d) => std::thread::sleep(d),
            _ => {}
        }
        self.gain(frames);
        if let Behavior::NaN { after } = self.behavior {
            if c >= after {
                let o0 = self.layout.output_range(0).start;
                self.mem[o0..o0 + 4].copy_from_slice(&f32::NAN.to_le_bytes());
            }
        }
        Ok(100)
    }

    fn reset(&mut self) -> Result<(), Fault> {
        Ok(())
    }
}
