//! `PatchEditor`: the patch-editing logic behind the UI, deliberately kept free of any `egui`
//! type so it's testable without a display (this container has none — see the crate's module
//! doc). Every mutation goes through `kabl_core::PatchLog::append`, so undo/redo (brief section
//! 4.1/6) comes for free rather than being a UI-layer concern re-implemented on top.

use kabl_core::{CableId, ModuleId, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2};
use kabl_modules::registry;

pub struct PatchEditor {
    log: PatchLog,
    next_module_id: ModuleId,
    next_cable_id: CableId,
    /// Bumped by every mutating call — the UI/audio host checks and clears this to know when a
    /// recompile+swap is due, rather than the editor reaching into `PatchEngine` itself (keeps
    /// this crate's core logic decoupled from the audio-engine wiring, same reasoning
    /// `VoiceAllocator` stayed decoupled from `CompiledPatch`).
    dirty: bool,
}

impl Default for PatchEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchEditor {
    pub fn new() -> Self {
        PatchEditor {
            log: PatchLog::new(),
            next_module_id: 1,
            next_cable_id: 1,
            dirty: false,
        }
    }

    /// Seeds the editor from an existing patch (e.g. `kabl_standalone::default_patch()`) by
    /// replaying it as `AddModule`/`Connect` ops through the real log — so the starting patch is
    /// itself undoable back to empty, not a special-cased starting state the op log doesn't know
    /// about.
    pub fn seed_from(patch: &PatchState) -> Self {
        let mut editor = Self::new();
        for (&id, m) in &patch.modules {
            editor.log.append(
                Op::AddModule {
                    id,
                    kind: m.kind.clone(),
                    pos: m.pos,
                },
                0,
                Source::User,
            );
            for (name, &value) in &m.params {
                editor.log.append(
                    Op::SetParam {
                        target: ParamTarget::Module {
                            id,
                            param: name.clone(),
                        },
                        value,
                    },
                    0,
                    Source::User,
                );
            }
            editor.next_module_id = editor.next_module_id.max(id + 1);
        }
        for (&id, c) in &patch.cables {
            editor.log.append(
                Op::Connect {
                    id,
                    from: c.from.clone(),
                    to: c.to.clone(),
                },
                0,
                Source::User,
            );
            editor.next_cable_id = editor.next_cable_id.max(id + 1);
        }
        editor.dirty = false; // seeding isn't a user edit -- nothing needs a swap for it yet.
        editor
    }

    pub fn state(&self) -> &PatchState {
        self.log.state()
    }

    pub fn can_undo(&self) -> bool {
        self.log.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.log.can_redo()
    }

    /// True if the patch has changed since the last `take_dirty()` call.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clears and returns the dirty flag — the audio host calls this once per check, so a swap
    /// gets triggered exactly once per batch of edits rather than once per individual op.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn append(&mut self, op: Op) {
        self.log.append(op, now_ms(), Source::User);
        self.dirty = true;
    }

    /// Adds a module of `kind` at `pos`. `kind` isn't validated against the registry here — an
    /// unknown kind produces a patch `compile()` will reject with `CompileError::UnknownKind`,
    /// exactly the same error path a corrupt saved file would hit; the editor doesn't need its
    /// own separate validation layer duplicating that.
    pub fn add_module(&mut self, kind: &str, pos: Vec2) -> ModuleId {
        let id = self.next_module_id;
        self.next_module_id += 1;
        self.append(Op::AddModule {
            id,
            kind: kind.to_string(),
            pos,
        });
        id
    }

    pub fn remove_module(&mut self, id: ModuleId) {
        if self.log.state().modules.contains_key(&id) {
            self.append(Op::RemoveModule { id });
        }
    }

    pub fn move_module(&mut self, id: ModuleId, pos: Vec2) {
        if self.log.state().modules.contains_key(&id) {
            self.append(Op::MoveModule { id, pos });
        }
    }

    pub fn set_param(&mut self, id: ModuleId, param: &str, value: f32) {
        self.append(Op::SetParam {
            target: ParamTarget::Module {
                id,
                param: param.to_string(),
            },
            value,
        });
    }

    /// Connects `from` (an output port) to `to` (an input port). No port-type/direction
    /// validation here for the same reason `add_module`'s kind isn't validated — `compile()`
    /// already rejects a cable naming a nonexistent port (`CompileError::UnknownPort`), so the
    /// editor doesn't need a second copy of that check. The UI layer (`lib.rs`'s `show`) only
    /// ever offers ports of the right direction as click targets in the first place, so this
    /// path is normally never exercised with a bad port outside of a test.
    pub fn connect(&mut self, from: PortRef, to: PortRef) -> CableId {
        let id = self.next_cable_id;
        self.next_cable_id += 1;
        self.append(Op::Connect { id, from, to });
        id
    }

    pub fn disconnect(&mut self, id: CableId) {
        if self.log.state().cables.contains_key(&id) {
            self.append(Op::Disconnect { id });
        }
    }

    pub fn undo(&mut self) -> bool {
        let did = self.log.undo();
        if did {
            self.dirty = true;
        }
        did
    }

    pub fn redo(&mut self) -> bool {
        let did = self.log.redo();
        if did {
            self.dirty = true;
        }
        did
    }

    /// Every known module kind, for the "add module" palette.
    pub fn known_kinds() -> &'static [&'static str] {
        registry::KNOWN_KINDS
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
