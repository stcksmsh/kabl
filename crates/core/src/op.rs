use serde::{Deserialize, Serialize};

pub type ModuleId = u64;
pub type CableId = u64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortRef {
    Module { id: ModuleId, port: String },
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
    SetCablePattern {
        id: CableId,
        steps: Vec<f32>,
    },
    MoveModule {
        id: ModuleId,
        pos: Vec2,
    },
    Snapshot {
        name: String,
    },
    Annotate {
        text: String,
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
