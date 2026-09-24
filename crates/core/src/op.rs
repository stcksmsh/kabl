use serde::{Deserialize, Serialize};

pub type ModuleId = u64;
pub type CableId = u64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// One end of a cable. `Module` is a signal jack. `Param` is a parameter knob as a modulation
/// destination: a cable into it is a modulation route whose settings (`amount`, `bypass`) live
/// in the cable's params. Schema v2; v1 files only contain `Module`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortRef {
    Module { id: ModuleId, port: String },
    Param { id: ModuleId, param: String },
}

impl PortRef {
    pub fn module_id(&self) -> ModuleId {
        match self {
            PortRef::Module { id, .. } | PortRef::Param { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamTarget {
    Module { id: ModuleId, param: String },
    Cable { id: CableId, param: String },
}

impl ParamTarget {
    pub fn same_target(&self, other: &ParamTarget) -> bool {
        self == other
    }
}

/// A single patch mutation. This is the complete vocabulary — see brief section 6.
/// Do not add variants without a `docs/decisions.md` entry (foundational decision, section 4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    AddModule {
        id: ModuleId,
        kind: String,
        pos: Vec2,
    },
    RemoveModule {
        id: ModuleId,
    },
    Connect {
        id: CableId,
        from: PortRef,
        to: PortRef,
    },
    Disconnect {
        id: CableId,
    },
    SetParam {
        target: ParamTarget,
        value: f32,
    },
    /// Removes a stored param value so the module default applies again. Exists so undoing the
    /// first `SetParam` on a param restores "absent", not a made-up 0.0. Schema v2.
    UnsetParam {
        target: ParamTarget,
    },
    SetCablePattern {
        id: CableId,
        steps: Vec<f32>,
    },
    MoveModule {
        id: ModuleId,
        pos: Vec2,
    },
    /// Sets (`Some`) or removes a text label on a module, keyed like a param (`pin.cutoff_hz`
    /// labels that pin). Presentation text; the compiler never reads it.
    SetLabel {
        id: ModuleId,
        key: String,
        text: Option<String>,
    },
    Snapshot {
        name: String,
    },
    Annotate {
        text: String,
    },
    /// Several ops applied in order as one user action, so one undo reverts all of them
    /// (repatch, delete with its cables, connect-and-configure). Schema v2.
    Group {
        ops: Vec<Op>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    User,
    Lesson,
    Midi,
    Director,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub t_ms: u64,
    pub op: Op,
    pub inverse: Op,
    pub source: Source,
}
