//! Stand-ins for code modules that cannot be loaded (declined by the user, invalid blob, ABI
//! mismatch, forbidden import, missing blob).
//!
//! The patch still opens: each failed module becomes a [`StandIn`] with the same ports and
//! params as its metadata (or, when the metadata is unreadable, as the patch uses it). Audio
//! inputs pass through to the audio outputs in order (first audio in to first audio out, and
//! so on); every other output is silent. The patch keeps the module's kind and its settings, and
//! [`super::host`] keeps the original blob, so saving writes back exactly what was loaded and a
//! host that can load the module restores everything.

use super::meta::CodeMeta;
use super::scope::{CodeHash, CodeScope};
use crate::info::{ModuleInfo, PortDirection};
use crate::module::Module;
use crate::{PortType, ProcessIo, QualityConfig};
use std::collections::BTreeMap;
use std::sync::RwLock;

/// Why a module is a stand-in: shown to the user with the module id and version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub kind: String,
    pub version: u32,
    pub cause: String,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "custom module {} v{} is not running ({}); it passes audio through and everything is kept",
            self.kind, self.version, self.cause
        )
    }
}

struct Registered {
    info: &'static ModuleInfo,
    report: Report,
}

/// Stand-ins by key (see [`super::standin_key`]), so each song keeps its own.
static STAND_INS: RwLock<BTreeMap<CodeHash, Registered>> = RwLock::new(BTreeMap::new());

pub(super) fn register(key: CodeHash, meta: &CodeMeta, report: Report) {
    STAND_INS.write().unwrap_or_else(|e| e.into_inner()).insert(
        key,
        Registered {
            info: meta.info(),
            report,
        },
    );
}

/// A fresh stand-in, if one is registered under `key`.
pub(super) fn create(key: &CodeHash) -> Option<Box<dyn Module>> {
    let s = STAND_INS.read().unwrap_or_else(|e| e.into_inner());
    s.get(key)
        .map(|r| Box::new(StandIn::new(r.info)) as Box<dyn Module>)
}

pub(super) fn info_for(key: &CodeHash) -> Option<&'static ModuleInfo> {
    STAND_INS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(key)
        .map(|r| r.info)
}

/// The stand-ins now in use without a scope (the latest of their kind), for the warning the
/// user sees.
pub fn reports() -> Vec<Report> {
    let s = STAND_INS.read().unwrap_or_else(|e| e.into_inner());
    s.iter()
        .filter(|(key, r)| super::latest_key(&r.report.kind) == Some(**key))
        .map(|(_, r)| r.report.clone())
        .collect()
}

/// The stand-ins the song `scope` describes.
pub fn reports_in(scope: &CodeScope) -> Vec<Report> {
    let s = STAND_INS.read().unwrap_or_else(|e| e.into_inner());
    s.iter()
        .filter(|(key, r)| scope.key(&r.report.kind) == Some(**key))
        .map(|(_, r)| r.report.clone())
        .collect()
}

/// Audio pass-through for one module instance. Allocates only in `new` (control thread).
pub struct StandIn {
    info: &'static ModuleInfo,
    /// `(input slot, output slot)` of each audio pass-through.
    pairs: Vec<(usize, usize)>,
    out_slots: usize,
}

impl StandIn {
    pub fn new(info: &'static ModuleInfo) -> StandIn {
        let slots = |dir: PortDirection| -> Vec<usize> {
            let mut at = 0;
            let mut audio = Vec::new();
            for p in info.ports.iter().filter(|p| p.direction == dir) {
                for w in 0..usize::from(p.width) {
                    if p.port_type == PortType::Audio {
                        audio.push(at + w);
                    }
                }
                at += usize::from(p.width);
            }
            audio
        };
        let pairs = slots(PortDirection::Input)
            .into_iter()
            .zip(slots(PortDirection::Output))
            .collect();
        StandIn {
            info,
            pairs,
            out_slots: info.slots(PortDirection::Output),
        }
    }
}

impl Module for StandIn {
    fn info(&self) -> &'static ModuleInfo {
        self.info
    }
    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        for s in 0..self.out_slots {
            io.output(s)[..n].fill(0.0);
        }
        for &(i, o) in &self.pairs {
            let src = io.input(i);
            let out = io.output(o);
            for (k, v) in out[..n].iter_mut().enumerate() {
                *v = src.at(k);
            }
        }
    }
    fn reset(&mut self) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
