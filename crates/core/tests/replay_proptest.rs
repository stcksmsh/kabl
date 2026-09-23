//! Property tests for the op log (brief section 6):
//!   - `replay(log) == state`
//!   - `undo^n(redo^n(state)) == state`, and every intermediate undo equals a replay of the
//!     remaining prefix
//!
//! Fuzzing raw `Op` values mostly produces no-ops (references to ids that don't exist), so this
//! generates small abstract `Action`s and interprets them against a live-id tracker, emitting
//! only `Op`s that are valid to append (skipping e.g. `SetParam` on a module that was never
//! added). That keeps the fuzzed sequences meaningful.

use kabl_core::{CableId, ModuleId, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Action {
    AddModule(u8),
    RemoveModule(u8),
    Connect(u8, u8, u8),
    Disconnect(u8),
    SetParam(u8, u8),
    /// Route amount on a live cable, or bypass when the second value is odd.
    SetCableParam(u8, u8),
    /// Disconnect a cable and connect a replacement as one grouped action (a repatch).
    Repatch(u8, u8),
    MoveModule(u8, i8, i8),
    Annotate,
}

const PARAM_NAMES: [&str; 2] = ["freq", "cutoff"];

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0u8..4).prop_map(Action::AddModule),
        (0u8..4).prop_map(Action::RemoveModule),
        (0u8..4, 0u8..4, 0u8..4).prop_map(|(c, f, t)| Action::Connect(c, f, t)),
        (0u8..4).prop_map(Action::Disconnect),
        (0u8..4, 0u8..2).prop_map(|(m, p)| Action::SetParam(m, p)),
        (0u8..4, 0u8..2).prop_map(|(c, p)| Action::SetCableParam(c, p)),
        (0u8..4, 0u8..4).prop_map(|(c, t)| Action::Repatch(c, t)),
        (0u8..4, -5i8..5i8, -5i8..5i8).prop_map(|(m, x, y)| Action::MoveModule(m, x, y)),
        Just(Action::Annotate),
    ]
}

fn apply_action(log: &mut PatchLog, action: &Action, t_ms: u64) {
    let live_modules: Vec<ModuleId> = log.state().modules.keys().copied().collect();
    let live_cables: Vec<CableId> = log.state().cables.keys().copied().collect();
    match *action {
        Action::AddModule(id) => {
            let id = id as ModuleId;
            if !live_modules.contains(&id) {
                log.append(
                    Op::AddModule {
                        id,
                        kind: "osc.va".into(),
                        pos: Vec2 { x: 0.0, y: 0.0 },
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::RemoveModule(id) => {
            let id = id as ModuleId;
            if live_modules.contains(&id) {
                log.append(Op::RemoveModule { id }, t_ms, Source::User);
            }
        }
        Action::Connect(cid, from, to) => {
            let cid = cid as CableId;
            let (from_id, to_id) = (from as ModuleId, to as ModuleId);
            if !live_cables.contains(&cid)
                && live_modules.contains(&from_id)
                && live_modules.contains(&to_id)
            {
                log.append(
                    Op::Connect {
                        id: cid,
                        from: PortRef::Module {
                            id: from_id,
                            port: "out".into(),
                        },
                        to: if cid % 2 == 1 {
                            PortRef::Param {
                                id: to_id,
                                param: "freq".into(),
                            }
                        } else {
                            PortRef::Module {
                                id: to_id,
                                port: "in".into(),
                            }
                        },
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::Disconnect(cid) => {
            let cid = cid as CableId;
            if live_cables.contains(&cid) {
                log.append(Op::Disconnect { id: cid }, t_ms, Source::User);
            }
        }
        Action::SetParam(mid, p) => {
            let mid = mid as ModuleId;
            if live_modules.contains(&mid) {
                let param = PARAM_NAMES[(p % 2) as usize].to_string();
                log.append(
                    Op::SetParam {
                        target: ParamTarget::Module { id: mid, param },
                        value: t_ms as f32 * 0.01,
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::SetCableParam(cid, p) => {
            let cid = cid as CableId;
            if live_cables.contains(&cid) {
                let param = if p % 2 == 0 { "amount" } else { "bypass" };
                log.append(
                    Op::SetParam {
                        target: ParamTarget::Cable {
                            id: cid,
                            param: param.into(),
                        },
                        value: t_ms as f32 * 0.001,
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::Repatch(cid, to) => {
            let cid = cid as CableId;
            let to_id = to as ModuleId;
            if let (Some(c), true) = (log.state().cables.get(&cid), live_modules.contains(&to_id)) {
                let from = c.from.clone();
                let new_id = 100 + t_ms;
                log.append(
                    Op::Group {
                        ops: vec![
                            Op::Disconnect { id: cid },
                            Op::Connect {
                                id: new_id,
                                from,
                                to: PortRef::Module {
                                    id: to_id,
                                    port: "in".into(),
                                },
                            },
                        ],
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::MoveModule(mid, x, y) => {
            let mid = mid as ModuleId;
            if live_modules.contains(&mid) {
                log.append(
                    Op::MoveModule {
                        id: mid,
                        pos: Vec2 {
                            x: x as f32,
                            y: y as f32,
                        },
                    },
                    t_ms,
                    Source::User,
                );
            }
        }
        Action::Annotate => {
            log.append(
                Op::Annotate {
                    text: "note".into(),
                },
                t_ms,
                Source::User,
            );
        }
    }
}

proptest! {
    #[test]
    fn replay_matches_incremental_state(
        actions in prop::collection::vec(action_strategy(), 0..80),
        ts_seed in prop::collection::vec(0u64..400, 80),
    ) {
        let mut log = PatchLog::new();
        let mut t_ms = 0u64;
        for (action, dt) in actions.iter().zip(ts_seed.iter()) {
            t_ms += dt;
            apply_action(&mut log, action, t_ms);
        }

        let mut replay = PatchState::new();
        for e in log.entries() {
            replay.apply(&e.op);
        }
        prop_assert_eq!(&replay, log.state());
    }

    #[test]
    fn undo_redo_roundtrip(
        actions in prop::collection::vec(action_strategy(), 0..80),
        ts_seed in prop::collection::vec(0u64..400, 80),
    ) {
        let mut log = PatchLog::new();
        let mut t_ms = 0u64;
        for (action, dt) in actions.iter().zip(ts_seed.iter()) {
            t_ms += dt;
            apply_action(&mut log, action, t_ms);
        }

        let final_state = log.state().clone();
        let n = log.entries().len();

        // Every undo step must land exactly on the state a fresh replay of the remaining prefix
        // produces: absent params stay absent, removed modules return with params and cables.
        for k in (0..n).rev() {
            prop_assert!(log.undo());
            let mut replay = PatchState::new();
            for e in &log.all_entries()[..k] {
                replay.apply(&e.op);
            }
            prop_assert_eq!(log.state(), &replay);
        }
        prop_assert!(!log.undo());
        prop_assert_eq!(log.state(), &PatchState::new());

        for _ in 0..n {
            prop_assert!(log.redo());
        }
        prop_assert!(!log.redo());

        prop_assert_eq!(log.state(), &final_state);
    }
}
