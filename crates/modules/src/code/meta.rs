//! Owned module metadata. Built-ins describe themselves with `&'static ModuleInfo`; a code module
//! declares its metadata at load time, so the strings and slices are owned (`CodeMeta`) and then
//! leaked exactly once per distinct metadata into a `&'static ModuleInfo`. The rest of the
//! program (compiler, UI, registry) keeps using `ModuleInfo` unchanged.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub struct CodePort {
    pub name: String,
    pub port_type: PortType,
    pub direction: PortDirection,
    pub width: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeParam {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: String,
    pub taper: Taper,
    pub smoothing_ms: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeMeta {
    /// Stable kind string, the module id stored in patches.
    pub kind: String,
    pub version: u32,
    pub name: String,
    pub explain: String,
    pub category: Category,
    pub rate: Rate,
    pub ports: Vec<CodePort>,
    pub params: Vec<CodeParam>,
    pub advanced: Vec<String>,
    pub width_units: u32,
}

impl CodeMeta {
    pub fn slots(&self, dir: PortDirection) -> usize {
        self.ports
            .iter()
            .filter(|p| p.direction == dir)
            .map(|p| usize::from(p.width))
            .sum()
    }

    /// Basic sanity the registry relies on: a kind, unique port and param names, sane widths and
    /// ranges. Returns the first problem as text for the load error.
    pub fn validate(&self) -> Result<(), String> {
        if self.kind.is_empty() {
            return Err("empty kind".into());
        }
        for (i, p) in self.ports.iter().enumerate() {
            if !(1..=2).contains(&p.width) {
                return Err(format!(
                    "port `{}` width {} (must be 1 or 2)",
                    p.name, p.width
                ));
            }
            if self.ports[..i]
                .iter()
                .any(|q| q.name == p.name && q.direction == p.direction)
            {
                return Err(format!("duplicate port `{}`", p.name));
            }
        }
        for (i, p) in self.params.iter().enumerate() {
            if !(p.min.is_finite() && p.max.is_finite() && p.default.is_finite()) || p.min >= p.max
            {
                return Err(format!("param `{}` has an invalid range", p.name));
            }
            if self.params[..i].iter().any(|q| q.name == p.name) {
                return Err(format!("duplicate param `{}`", p.name));
            }
        }
        Ok(())
    }

    fn leak_info(&self) -> &'static ModuleInfo {
        fn s(x: &str) -> &'static str {
            Box::leak(x.to_owned().into_boxed_str())
        }
        let ports: Vec<PortInfo> = self
            .ports
            .iter()
            .map(|p| PortInfo {
                name: s(&p.name),
                port_type: p.port_type,
                direction: p.direction,
                width: p.width,
            })
            .collect();
        let params: Vec<ParamInfo> = self
            .params
            .iter()
            .map(|p| ParamInfo {
                name: s(&p.name),
                min: p.min,
                max: p.max,
                default: p.default,
                unit: s(&p.unit),
                taper: p.taper,
                smoothing_ms: p.smoothing_ms,
            })
            .collect();
        let advanced: Vec<&'static str> = self.advanced.iter().map(|a| s(a)).collect();
        Box::leak(Box::new(ModuleInfo {
            kind: s(&self.kind),
            name: s(&self.name),
            category: self.category,
            rate: self.rate,
            explain: s(&self.explain),
            lesson: None,
            requires: &[],
            ports: Box::leak(ports.into_boxed_slice()),
            params: Box::leak(params.into_boxed_slice()),
            quality: QualitySupport::default(),
            skin: None,
            width_units: self.width_units,
            advanced: Box::leak(advanced.into_boxed_slice()),
        }))
    }

    /// The `&'static ModuleInfo` for this metadata. Identical metadata (same module reloaded)
    /// reuses the first leak, so reloading a patch does not grow memory.
    pub fn info(&self) -> &'static ModuleInfo {
        static LEAKED: Mutex<Vec<(CodeMeta, &'static ModuleInfo)>> = Mutex::new(Vec::new());
        let mut all = LEAKED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, info)) = all.iter().find(|(m, _)| m == self) {
            return info;
        }
        let info = self.leak_info();
        all.push((self.clone(), info));
        info
    }
}
