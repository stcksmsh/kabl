//! D12 PR 0: songs sharing a process (kabl instances in one plug-in host) never share code.
//! Song A runs `test.scope` v1, song B v2, song C embeds a copy that was refused and became a
//! stand-in. Every engine renders its own code, before and after the others are loaded.

use basedrop::Collector;
use kabl_core::{Op, ParamTarget, PatchLog, PortRef, Source, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::code::{host, CodeScope};

const SR: f32 = 48000.0;
const KIND: &str = "test.scope";

/// A voice-rate module that outputs the constant `level` whatever its input (a stand-in passes
/// the input through instead), so each version is told apart by the level it renders.
fn module(version: u32, level: f32) -> Vec<u8> {
    module_full(KIND, version, level, "in")
}

/// [`module`] with its own kind and input port name.
fn module_full(kind: &str, version: u32, level: f32, input: &str) -> Vec<u8> {
    let meta = format!(
        "kind {kind}\\nversion {version}\\nabi_minor 1\\nname T\\nexplain Test fixture.\\ncategory filter\\n\
         rate voice\\nwidth 4\\nport in audio {input} 1\\nport out audio out 1\\nparam gain 0 4 1 linear 0 x\\n"
    );
    wat::parse_str(format!(
        r#"(module
 (@custom "kabl.meta" "{meta}")
 (memory (export "memory") 1)
 (func (export "init") (param f32 i32) (result i32)
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 8256))
  (i32.store (i32.const 8) (i32.const 16448)) (i32.store (i32.const 12) (i32.const 16456))
  (i32.store (i32.const 16) (i32.const 4)) (i32.store (i32.const 20) (i32.const 2048))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 1))
  (i32.const 0))
 (func (export "reset"))
 (func (export "process") (param $n i32) (local $i i32)
  (block $done (loop $l
   (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
   (f32.store (i32.add (i32.const 8256) (i32.shl (local.get $i) (i32.const 2))) (f32.const {level}))
   (local.set $i (i32.add (local.get $i) (i32.const 1)))
   (br $l))))
)"#
    ))
    .unwrap()
}

fn song() -> PatchLog {
    let mut log = PatchLog::new();
    let mut t = 0;
    let mut op = |log: &mut PatchLog, o: Op| {
        t += 1;
        log.append(o, t, Source::User)
    };
    for (id, kind) in [(1, "midi.in"), (2, "osc.va"), (3, KIND), (4, "out")] {
        op(
            &mut log,
            Op::AddModule {
                id,
                kind: kind.into(),
                pos: Vec2 { x: 0.0, y: 0.0 },
            },
        );
    }
    let p = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    for (cid, (f, fp, to, tp)) in [
        (1, "pitch", 2, "pitch"),
        (2, "out", 3, "in"),
        (3, "out", 4, "in"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (f, fp, to, tp))| (i as u64 + 1, (f, fp, to, tp)))
    {
        op(
            &mut log,
            Op::Connect {
                id: cid,
                from: p(f, fp),
                to: p(to, tp),
            },
        );
    }
    op(
        &mut log,
        Op::SetParam {
            target: ParamTarget::Module {
                id: 3,
                param: "gain".into(),
            },
            value: 1.0,
        },
    );
    log
}

/// The mean of the last blocks of the left channel of a held note.
fn level(engine: &mut PatchEngine, blocks: usize) -> f32 {
    engine.key(KeyEvent::On {
        note: 57,
        velocity: 100,
    });
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut tail = Vec::new();
    for b in 0..blocks {
        engine.process_block(&mut l, &mut r);
        if b + 16 >= blocks {
            tail.extend_from_slice(&l);
        }
    }
    assert!(tail.iter().all(|v| v.is_finite()));
    tail.iter().sum::<f32>() / tail.len() as f32
}

#[test]
fn engines_in_one_process_keep_their_own_version_and_a_refused_copy_stays_its_own() {
    let (v1, v2, v3) = (module(1, 0.25), module(2, 0.5), module(3, 0.75));
    let (a, b) = (
        host::register_blob(&v1).unwrap(),
        host::register_blob(&v2).unwrap(),
    );
    let (scope_a, scope_b) = (
        CodeScope::new().with(KIND, a.key),
        CodeScope::new().with(KIND, b.key),
    );
    let log = song();
    let c = Collector::new();
    let h = c.handle();
    let mut ea = PatchEngine::new_scoped(&h, log.state(), SR, 4, &scope_a).unwrap();
    let mut eb = PatchEngine::new_scoped(&h, log.state(), SR, 4, &scope_b).unwrap();

    // song C embeds v3, and the user refused it: a stand-in, registered after A and B started
    let (_, key_c) = host::register_stand_in_keyed(KIND, 3, Some(&v3), "declined", None);
    let scope_c = CodeScope::new().with(KIND, key_c);
    let mut ec = PatchEngine::new_scoped(&h, log.state(), SR, 4, &scope_c).unwrap();

    let (la, lb, lc) = (
        level(&mut ea, 200),
        level(&mut eb, 200),
        level(&mut ec, 200),
    );
    assert!(la > 0.01, "song A renders its v1 (got {la})");
    assert!(
        (lb / la - 2.0).abs() < 0.05,
        "song B renders v2, twice v1's level ({la} vs {lb})"
    );
    assert!(
        lc.abs() < la * 0.2,
        "song C is a pass-through stand-in ({lc})"
    );

    // A graph rebuild after the refused copy arrived (an edit, a device restart) still gives
    // each song its own code: this is what #137 broke for song A.
    let rebuilt_a = PatchEngine::new_scoped(&h, log.state(), SR, 4, &scope_a).unwrap();
    let rebuilt_b = PatchEngine::new_scoped(&h, log.state(), SR, 4, &scope_b).unwrap();
    let (mut ra, mut rb) = (rebuilt_a, rebuilt_b);
    assert!((level(&mut ra, 200) - la).abs() < 1e-4);
    assert!((level(&mut rb, 200) - lb).abs() < 1e-4);
}

/// A composite whose interface binds the older blob's `in` port validates for the song that
/// runs that blob, while the newer blob (no such port) is the latest one in the process.
#[test]
fn a_composite_validates_against_its_own_blobs_interface() {
    use kabl_core::{Composite, Definition, Exposure, ModuleState, PatchState};
    use kabl_engine::compile::{compile_scoped, validate_composites, validate_composites_scoped};
    let kind = "test.scope.iface";
    let a = host::register_blob(&module_full(kind, 1, 0.25, "in")).unwrap();
    let b = host::register_blob(&module_full(kind, 2, 0.25, "audio_in")).unwrap(); // the latest
    let scope_a = CodeScope::new().with(kind, a.key);
    let scope_b = CodeScope::new().with(kind, b.key);
    let mut st = PatchState::new();
    st.modules.insert(
        1,
        ModuleState {
            kind: kind.into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            params: Default::default(),
        },
    );
    let wrap = Composite {
        name: "Wrap".into(),
        help: String::new(),
        definition: Definition {
            id: "test.wrap".into(),
            version: 1,
        },
        parent: None,
        pos: Vec2 { x: 0.0, y: 0.0 },
        members: [1].into_iter().collect(),
        ports: [(
            0,
            Exposure {
                label: "In".into(),
                target: PortRef::Module {
                    id: 1,
                    port: "in".into(),
                },
            },
        )]
        .into_iter()
        .collect(),
        controls: Default::default(),
        next_interface: 1,
        panel: None,
    };
    st.composites.insert(1, wrap);
    assert!(validate_composites_scoped(&st, &scope_a).is_ok());
    assert!(validate_composites_scoped(&st, &scope_b).is_err());
    assert!(
        validate_composites(&st).is_err(),
        "without a scope the latest blob (no `in` port) is used"
    );
    assert!(compile_scoped(&st, SR, 2, &scope_a).is_ok());
    assert!(compile_scoped(&st, SR, 2, &scope_b).is_err());
}
