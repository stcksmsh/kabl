//! Custom-module code in the CLAP state: each instance loads and saves its own code, so kabl
//! instances in one host process never share modules.
use super::*;
use kabl_core::embedded::Embedded;
use kabl_core::{Op, ParamTarget, PatchLog, PortRef, Source, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;

const KIND: &str = "test.scope";

/// A voice-rate module that outputs the constant `level` whatever its input (a stand-in passes
/// the input through instead), so each version is told apart by the level it renders.
fn module(kind: &str, version: u32, level: f32) -> Vec<u8> {
    let meta = format!(
        "kind {kind}\\nversion {version}\\nabi_minor 1\\nname T\\nexplain Test fixture.\\ncategory filter\\n\
         rate voice\\nwidth 4\\nport in audio in 1\\nport out audio out 1\\nparam gain 0 4 1 linear 0 x\\n"
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
        (1u64, (1, "pitch", 2, "pitch")),
        (2, (2, "out", 3, "in")),
        (3, (3, "out", 4, "in")),
    ] {
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

/// A project state whose song uses `KIND` and embeds `bytes` for it.
fn state_with(bytes: Vec<u8>, version: u32) -> SoundState {
    let host = tests::instrument();
    let mut state = host.shared.snapshot();
    state.patch = song().state().clone();
    state.lanes = Default::default();
    let mut code = Embedded::new();
    let hash = code.add_blob(KIND, version, bytes).unwrap();
    code.bind(3, hash).unwrap();
    state.version = 6;
    state.code = Some(sound_state::encode_code(&code));
    state
}

/// The mean of the last blocks of the left channel of a held note.
fn level(p: &mut Instrument) -> f32 {
    p.accept_loads();
    let engine = &mut p.session.as_mut().unwrap().engine;
    engine.key(KeyEvent::On {
        note: 57,
        velocity: 100,
    });
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut tail = Vec::new();
    for b in 0..200 {
        engine.process_block(&mut l, &mut r);
        if b >= 184 {
            tail.extend_from_slice(&l);
        }
    }
    assert!(tail.iter().all(|v| v.is_finite()));
    tail.iter().sum::<f32>() / tail.len() as f32
}

#[test]
fn instances_in_one_process_run_their_own_code_and_a_refused_copy_is_a_stand_in() {
    let (mut a, mut b, mut c) = (
        tests::instrument(),
        tests::instrument(),
        tests::instrument(),
    );
    a.shared.load(state_with(module(KIND, 1, 0.25), 1)).unwrap();
    b.shared.load(state_with(module(KIND, 2, 0.5), 2)).unwrap();
    // A blob whose own metadata names another module cannot load for this song: stand-in.
    c.shared
        .load(state_with(module("test.other", 3, 0.75), 3))
        .unwrap();
    let (la, lb, lc) = (level(&mut a), level(&mut b), level(&mut c));
    assert!(la > 0.01, "instance A renders its v1 (got {la})");
    assert!(
        (lb / la - 2.0).abs() < 0.05,
        "instance B renders v2, twice v1's level ({la} vs {lb})"
    );
    assert!(lc.abs() < la * 0.2, "instance C is a stand-in ({lc})");
    // Loading the others did not change instance A: it loads again and renders the same.
    a.shared.load(state_with(module(KIND, 1, 0.25), 1)).unwrap();
    assert!((level(&mut a) - la).abs() < 1e-4);
}

#[test]
fn code_and_module_settings_round_trip_through_the_state_bit_exactly() {
    use kabl_modules::code::{host, state::SavedState, InstanceConfig, WasmEngine, WasmiEngine};
    host::register_examples().unwrap();
    let p = tests::instrument();
    let mut editor = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let (leaf, _) =
        kabl_ui::code_examples::add(&mut editor, "demo.bitfilter", Vec2 { x: 5., y: 5. }).unwrap();
    let mut state = p.shared.snapshot();
    state.patch = editor.state().clone();
    // The saved settings the file would carry: the module's own state.
    let mut fresh = WasmiEngine::new()
        .compile(host::EXAMPLES[1].wasm)
        .unwrap()
        .instantiate(&InstanceConfig::new(48_000.0, 2048))
        .unwrap();
    let saved = kabl_modules::code::state::save(&mut *fresh)
        .unwrap()
        .unwrap();
    let mut code = kabl_standalone::code_load::embedded_for(
        &state.patch,
        &kabl_modules::code::CodeScope::new(),
        std::time::Duration::ZERO,
    );
    let version = code.instance_blob(leaf).unwrap().version;
    code.keep_state(leaf, version, SavedState::to_kept(&saved))
        .unwrap();
    state.version = 6;
    state.code = Some(sound_state::encode_code(&code));
    let first = serde_json::to_vec(&state).unwrap();

    let a = tests::instrument();
    a.shared.load(SoundState::decode(&first).unwrap()).unwrap();
    let second = serde_json::to_vec(&a.shared.snapshot_for_save().0).unwrap();
    assert_eq!(
        first, second,
        "state, code and module settings survive a save"
    );
    let b = tests::instrument();
    b.shared.load(SoundState::decode(&second).unwrap()).unwrap();
    assert_eq!(
        second,
        serde_json::to_vec(&b.shared.snapshot_for_save().0).unwrap()
    );
}

#[test]
fn a_state_with_a_damaged_code_section_is_refused_and_keeps_the_sound() {
    let p = tests::instrument();
    let before = serde_json::to_vec(&p.shared.snapshot()).unwrap();
    let mut state = state_with(module(KIND, 1, 0.25), 1);
    let code = state.code.take().unwrap();
    state.code = Some(format!("zz{}", &code[2..]));
    assert!(p.shared.load(state).is_err());
    let mut state = state_with(module(KIND, 1, 0.25), 1);
    let code = state.code.take().unwrap();
    state.code = Some(code[..code.len() - 2].to_string());
    assert!(p.shared.load(state).is_err());
    assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), before);
}

#[test]
fn a_state_without_code_pins_the_version_registered_when_it_loads() {
    let kind = "test.pin";
    // The kind is installed in the process (as another instance's state would have done);
    // this instance's state, saved before code was embedded, carries none.
    kabl_modules::code::host::register_blob_as(&module(kind, 1, 0.25), kind, 1).unwrap();
    let mut legacy = state_with(module(KIND, 1, 0.25), 1);
    legacy.patch.modules.get_mut(&3).unwrap().kind = kind.into();
    legacy.code = None;
    legacy.version = 5;
    let mut l = tests::instrument();
    l.shared.load(legacy).unwrap();
    // Another instance now installs a different version of the same kind.
    kabl_modules::code::host::register_blob_as(&module(kind, 2, 0.5), kind, 2).unwrap();
    let saved = l.shared.snapshot_for_save().0;
    let code = saved
        .embedded()
        .unwrap()
        .expect("the state it saves embeds its code");
    assert_eq!(code.blobs().next().unwrap().1.version, 1);
    assert!(level(&mut l) < 0.3, "still renders version 1");
}

#[test]
fn a_failed_browser_load_keeps_the_previous_scope() {
    let p = tests::instrument();
    let mut c = p.shared.control.lock().unwrap();
    // The candidate song does not prepare (too many modules).
    for _ in 0..130 {
        c.editor.add_module("osc.va", Vec2 { x: 0., y: 0. });
    }
    c.view.loaded = true;
    c.view.open_scope = Some(kabl_modules::code::CodeScope::new().with("test.x", [1; 32]));
    assert!(!c.pump());
    assert!(
        c.scope.key("test.x").is_none(),
        "the rejected scope is not installed"
    );
}

#[test]
fn a_state_refused_for_another_reason_registers_none_of_its_code() {
    let p = tests::instrument();
    let kind = "test.unregistered";
    let mut state = state_with(module(kind, 1, 0.25), 1);
    // Same blob, bound under another kind name the song uses.
    state.patch.modules.get_mut(&3).unwrap().kind = kind.into();
    let mut code = Embedded::new();
    let hash = code.add_blob(kind, 1, module(kind, 1, 0.25)).unwrap();
    code.bind(3, hash).unwrap();
    state.code = Some(sound_state::encode_code(&code));
    state.output_gain = 7.0;
    assert!(p.shared.load(state).is_err());
    assert!(kabl_modules::code::host::blob_for(kind).is_none());
}

#[test]
fn a_state_whose_code_exceeds_the_load_cap_does_not_validate() {
    let mut state = state_with(module(KIND, 1, 0.25), 1);
    state.code = Some("0".repeat(2 * sound_state::MAX_CODE_BYTES + 2));
    assert!(state.validate().is_err());
}
