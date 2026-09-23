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

    /// Wraps an already-built `PatchLog` (e.g. loaded from disk via `kabl_core::load`) directly,
    /// preserving its real history — unlike `seed_from`, which *replays* a bare `PatchState` as
    /// fresh ops (right for a hardcoded starting patch that never had real history to begin
    /// with, wrong for a file that already has genuine timestamps/sources worth keeping).
    pub fn from_log(log: PatchLog) -> Self {
        let next_module_id = log.state().modules.keys().max().copied().unwrap_or(0) + 1;
        let next_cable_id = log.state().cables.keys().max().copied().unwrap_or(0) + 1;
        PatchEditor {
            log,
            next_module_id,
            next_cable_id,
            dirty: false,
        }
    }

    /// The full op log — what `kabl_core::save` needs to persist real undo history, not just a
    /// snapshot of `state()`.
    pub fn log(&self) -> &PatchLog {
        &self.log
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

    /// True if the patch has changed in a way that affects audio since the last `take_dirty()`
    /// call. Layout-only edits (moves) never set it.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Explicitly flags the patch as needing a recompile+swap without going through one of the
    /// `Op`-appending mutators — for a caller that replaced `*self` wholesale (e.g. `*editor =
    /// PatchEditor::from_log(...)` after loading a file mid-session) and needs the audio host to
    /// notice, unlike the initial startup seed (`seed_from`/`from_log` both start clean on
    /// purpose, since their caller compiles the seeded patch directly rather than relying on
    /// `take_dirty()`).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clears and returns the dirty flag — the audio host calls this once per check, so a swap
    /// gets triggered exactly once per batch of edits rather than once per individual op.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn append(&mut self, op: Op) {
        self.dirty |= affects_audio(&op);
        self.log.append(op, now_ms(), Source::User);
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

    /// One frame of a drag gesture on `target`. `first` starts a new undo entry; later frames
    /// merge into it however slowly the hand moves, so the whole drag undoes as one step.
    pub fn set_param_gesture(&mut self, target: ParamTarget, value: f32, first: bool) {
        let op = Op::SetParam { target, value };
        self.dirty = true;
        if first {
            self.log.append_new(op, now_ms(), Source::User);
        } else {
            self.log.append_continuing(op, now_ms(), Source::User);
        }
    }

    /// Cancels the gesture whose first frame was the last edit: its value is reverted and no
    /// undo or redo entry remains.
    pub fn cancel_gesture(&mut self) {
        if self.log.discard_last() {
            self.dirty = true;
        }
    }

    /// Adds a modulation route from output `from` to param `param` of module `to`. The route
    /// starts at the engine's default amount (+25 %), not bypassed. Knobs take any number of
    /// routes.
    pub fn connect_route(&mut self, from: PortRef, to: ModuleId, param: &str) -> CableId {
        self.connect(
            from,
            PortRef::Param {
                id: to,
                param: param.to_string(),
            },
        )
    }

    /// Sets a route's signed amount (fraction of knob travel, -1..1). `first` as in
    /// `set_param_gesture`; a single click-edit passes `true`.
    pub fn set_route_amount(&mut self, cable: CableId, amount: f32, first: bool) {
        self.set_param_gesture(
            ParamTarget::Cable {
                id: cable,
                param: "amount".into(),
            },
            amount.clamp(-1.0, 1.0),
            first,
        );
    }

    pub fn set_route_bypass(&mut self, cable: CableId, bypass: bool) {
        self.append(Op::SetParam {
            target: ParamTarget::Cable {
                id: cable,
                param: "bypass".into(),
            },
            value: if bypass { 1.0 } else { 0.0 },
        });
    }

    /// Connects `from` (an output port) to `to` (an input port). No port-type/direction
    /// validation here for the same reason `add_module`'s kind isn't validated — `compile()`
    /// already rejects a cable naming a nonexistent port (`CompileError::UnknownPort`), so the
    /// editor doesn't need a second copy of that check. The UI layer (`lib.rs`'s `show`) only
    /// ever offers ports of the right direction as click targets in the first place, so this
    /// path is normally never exercised with a bad port outside of a test.
    ///
    /// A jack input holds one cable: connecting into an occupied jack replaces the old cable,
    /// and the replacement undoes as one step. Param destinations (routes) accumulate.
    pub fn connect(&mut self, from: PortRef, to: PortRef) -> CableId {
        let id = self.next_cable_id;
        self.next_cable_id += 1;
        let mut ops: Vec<Op> = Vec::new();
        if matches!(to, PortRef::Module { .. }) {
            for (&old, c) in &self.log.state().cables {
                if c.to == to {
                    ops.push(Op::Disconnect { id: old });
                }
            }
        }
        let connect = Op::Connect { id, from, to };
        if ops.is_empty() {
            self.append(connect);
        } else {
            ops.push(connect);
            self.append(Op::Group { ops });
        }
        id
    }

    /// Sets which params of module `id` are on its face (`on_face`, aligned with the module's
    /// declared params) as one undoable step. A choice equal to the module default is stored as
    /// absent. Presentation only: never rebuilds audio. No-op when nothing changes.
    pub fn set_primary(&mut self, id: ModuleId, on_face: &[bool]) {
        let Some(m) = self.log.state().modules.get(&id) else {
            return;
        };
        let Some(info) = registry::info_for(&m.kind) else {
            return;
        };
        let mut ops = Vec::new();
        for (p, &on) in info.params.iter().zip(on_face) {
            let key = crate::rack::face_key(p.name);
            let default = !info.advanced.contains(&p.name);
            let target = ParamTarget::Module {
                id,
                param: key.clone(),
            };
            let stored = m.params.get(&key).map(|&v| v >= 0.5);
            if on == default && stored.is_some() {
                ops.push(Op::UnsetParam { target });
            } else if on != default && stored != Some(on) {
                ops.push(Op::SetParam {
                    target,
                    value: if on { 1.0 } else { 0.0 },
                });
            }
        }
        if !ops.is_empty() {
            self.append(Op::Group { ops });
        }
    }

    pub fn disconnect(&mut self, id: CableId) {
        if self.log.state().cables.contains_key(&id) {
            self.append(Op::Disconnect { id });
        }
    }

    pub fn undo(&mut self) -> bool {
        let audio = self.log.undo_entry().is_some_and(|e| affects_audio(&e.op));
        let did = self.log.undo();
        self.dirty |= did && audio;
        did
    }

    pub fn redo(&mut self) -> bool {
        let audio = self.log.redo_entry().is_some_and(|e| affects_audio(&e.op));
        let did = self.log.redo();
        self.dirty |= did && audio;
        did
    }

    /// Every known module kind, for the "add module" palette.
    pub fn known_kinds() -> &'static [&'static str] {
        registry::KNOWN_KINDS
    }
}

/// Whether applying `op` can change what the compiler produces. Moving a module, choosing its
/// face controls or annotating the log is presentation/history only and must not rebuild audio.
pub fn affects_audio(op: &Op) -> bool {
    match op {
        Op::MoveModule { .. } | Op::Annotate { .. } | Op::Snapshot { .. } => false,
        Op::SetParam {
            target: ParamTarget::Module { param, .. },
            ..
        }
        | Op::UnsetParam {
            target: ParamTarget::Module { param, .. },
        } if param.starts_with(crate::rack::FACE_PREFIX) => false,
        Op::Group { ops } => ops.iter().any(affects_audio),
        _ => true,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
