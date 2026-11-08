//! D12 PR 0: the host side of the ABI preview. Unknown meta keys, the `abi_minor` gate, the
//! `layout_ext` table, saved settings through the transfer buffer (`state_save`, `state_load`,
//! `migrate`), non-finite input, and the code registry keyed by blob hash.
//!
//! Modules are WAT text; the example blobs are the SDK-built ones.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_modules::code::state::{capture, save, Restored, SavedState, StateError};
use kabl_modules::code::{
    host, CodeGroup, CodeScope, InstanceConfig, LoadError, WasmEngine, WasmiEngine,
};
use kabl_modules::{Module, ProcessIo, QualityConfig, QualityTier, Signal};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[global_allocator]
static A: AllocDisabler = AllocDisabler;

const SR: f32 = 48_000.0;
const N: usize = 64;

/// Registry entries are process-wide: tests that register blobs take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn meta(kind: &str, version: u32, extra: &str) -> String {
    format!(
        "kind {kind}\\nversion {version}\\n{extra}name T\\nexplain test\\ncategory utility\\nrate voice\\n\
         port in audio in 1\\nport out audio out 1\\n"
    )
}

/// How the test module's settings behave. The setting is one u32 at address 16456 (`cell`),
/// saved as four bytes at the transfer buffer (16512, 16 bytes, layout version 2).
struct Fixture<'a> {
    meta: String,
    /// `state_save` body (result i32), `state_load` body (params $len $ver, result i32).
    save: &'a str,
    load: &'a str,
    /// `migrate` body (params $len $ver, result i32); `None`: not exported.
    migrate: Option<&'a str>,
    xfer_cap: u32,
    /// Output `in * cell_as_gain`? No: pass-through plus `cell` as a DC offset, so a loaded
    /// setting is audible.
    ext: bool,
    out: &'a str,
}

const SAVE_OK: &str = "(i32.store (i32.const 16512) (i32.load (i32.const 16456))) (i32.const 4)";
const LOAD_OK: &str = "(if (i32.ne (local.get $len) (i32.const 4)) (then (return (i32.const 0))))\
     (i32.store (i32.const 16456) (i32.load (i32.const 16512))) (i32.const 1)";
/// Copies input to output frame by frame.
const COPY: &str =
    "(local $i i32) (block $d (loop $l (br_if $d (i32.ge_u (local.get $i) (local.get $n)))\
     (f32.store (i32.add (i32.const 8256) (i32.shl (local.get $i) (i32.const 2)))\
       (f32.load (i32.add (i32.const 64) (i32.shl (local.get $i) (i32.const 2)))))\
     (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $l)))";

impl<'a> Fixture<'a> {
    fn new(meta: String) -> Self {
        Fixture {
            meta,
            save: SAVE_OK,
            load: LOAD_OK,
            migrate: None,
            xfer_cap: 16,
            ext: true,
            out: COPY,
        }
    }

    fn wasm(&self) -> Vec<u8> {
        let migrate = self.migrate.map_or(String::new(), |b| {
            format!(
                r#"(func (export "migrate") (param $len i32) (param $ver i32) (result i32) {b})"#
            )
        });
        let ext = if self.ext {
            format!(
                r#"(func (export "layout_ext") (result i32)
                    (i32.store (i32.const 16480) (i32.const 3)) (i32.store (i32.const 16484) (i32.const 16512))
                    (i32.store (i32.const 16488) (i32.const {})) (i32.store (i32.const 16492) (i32.const 2))
                    (i32.const 16480))
                 (func (export "state_save") (result i32) {})
                 (func (export "state_load") (param $len i32) (param $ver i32) (result i32) {})"#,
                self.xfer_cap, self.save, self.load
            )
        } else {
            String::new()
        };
        wat::parse_str(format!(
            r#"(module
 (@custom "kabl.meta" "{}")
 (memory (export "memory") 1)
 (func (export "init") (param f32 i32) (result i32)
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 8256))
  (i32.store (i32.const 8) (i32.const 16448)) (i32.store (i32.const 12) (i32.const 16456))
  (i32.store (i32.const 16) (i32.const 4)) (i32.store (i32.const 20) (i32.const 2048))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 0))
  (i32.const 0))
 (func (export "reset") (i32.store (i32.const 16456) (i32.const 0)))
 (func (export "process") (param $n i32) {})
 {ext} {migrate})"#,
            self.meta, self.out
        ))
        .unwrap()
    }
}

fn engine() -> WasmiEngine {
    WasmiEngine::new()
}

fn group(bytes: &[u8]) -> Arc<CodeGroup> {
    let compiled = engine().compile(bytes).unwrap();
    CodeGroup::new(
        compiled,
        InstanceConfig::new(SR, N as u32),
        1_000_000,
        Duration::from_secs(1),
    )
    .unwrap()
}

fn prepared(mut m: kabl_modules::code::CodeModule) -> kabl_modules::code::CodeModule {
    m.prepare(
        SR,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    m
}

fn run(m: &mut kabl_modules::code::CodeModule, input: &[f32; N]) -> [f32; N] {
    let sig = [Signal::Buffer(input)];
    let mut out = [0f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &[], N);
        m.process(&mut io);
    }
    out
}

// ---- meta keys and the abi_minor gate

#[test]
fn unknown_meta_keys_are_ignored_and_listed_for_the_validator() {
    let text = meta("test.keys", 1, "x-future some value\\nhelp gain Louder\\n");
    let f = Fixture::new(text.clone());
    assert!(engine().compile(&f.wasm()).is_ok());
    let plain = text.replace("\\n", "\n");
    assert_eq!(
        kabl_modules::code::wasmi_engine::unknown_keys(&plain),
        ["x-future", "help"]
    );
}

#[test]
fn abi_minor_above_the_hosts_is_refused_with_a_clear_message() {
    let ok = Fixture::new(meta("test.minor", 1, "abi_minor 1\\n"));
    assert!(engine().compile(&ok.wasm()).is_ok());
    let none = Fixture::new(meta("test.minor", 1, ""));
    assert!(
        engine().compile(&none.wasm()).is_ok(),
        "no line is revision 0"
    );
    let newer = Fixture::new(meta(
        "test.minor",
        1,
        "abi_minor 9\\nneeds-new-thing yes\\n",
    ));
    let Err(e) = engine().compile(&newer.wasm()) else {
        panic!("a module needing revision 9 must be refused");
    };
    assert_eq!(e, LoadError::AbiMinor { module: 9, host: 1 });
    let msg = e.to_string();
    assert!(
        msg.contains("revision 9") && msg.contains("Update kabl"),
        "{msg}"
    );
    let bad = Fixture::new(meta("test.minor", 1, "abi_minor x\\n"));
    assert!(matches!(
        engine().compile(&bad.wasm()),
        Err(LoadError::Invalid(_))
    ));
}

// ---- layout_ext

#[test]
fn a_module_without_layout_ext_has_no_saved_settings() {
    let mut f = Fixture::new(meta("test.noext", 1, ""));
    f.ext = false;
    let wasm = f.wasm();
    let g = group(&wasm);
    // the state calls are not there: saving yields nothing, restoring starts at defaults
    assert_eq!(save(&mut *instance_for(&wasm)), Ok(None));
    let (_, r) = g
        .restore(&SavedState {
            state_version: 2,
            bytes: vec![1, 0, 0, 0],
        })
        .unwrap();
    assert!(matches!(r, Restored::Defaults { .. }));
}

#[test]
fn a_transfer_buffer_over_64_kib_is_refused_at_load() {
    let mut f = Fixture::new(meta("test.bigxfer", 1, ""));
    f.xfer_cap = 65 * 1024;
    let g = group(&f.wasm());
    let Err(e) = g.fill(1) else {
        panic!("over the cap");
    };
    assert!(
        matches!(&e, LoadError::Layout(m) if m.contains("65") || m.contains("66560")),
        "{e}"
    );
    f.xfer_cap = 60_000; // under the cap, but it runs past the end of the one page
    let g = group(&f.wasm());
    assert!(matches!(g.fill(1), Err(LoadError::Layout(m)) if m.contains("outside")));
}

// ---- saved settings

/// A bare instance (the pool's build path), for calling `save` directly.
fn instance_for(wasm: &[u8]) -> Box<dyn kabl_modules::code::WasmInstance> {
    let c = engine().compile(wasm).unwrap();
    c.instantiate(&InstanceConfig::new(SR, N as u32)).unwrap()
}

#[test]
fn settings_round_trip_bit_exact_through_the_transfer_buffer() {
    let f = Fixture::new(meta("test.state", 1, ""));
    let wasm = f.wasm();
    let g = group(&wasm);
    let saved = SavedState {
        state_version: 2,
        bytes: 0xDEAD_BEEFu32.to_le_bytes().to_vec(),
    };
    let (m, r) = g.restore(&saved).unwrap();
    assert_eq!(r, Restored::Loaded);
    drop(m);
    // load into a bare instance by hand, then save: identical bytes
    let mut inst = instance_for(&wasm);
    let x = inst.state_xfer().unwrap();
    assert_eq!((x.cap, x.state_version), (16, 2));
    inst.memory()[x.addr..x.addr + 4].copy_from_slice(&saved.bytes);
    assert_eq!(inst.call_state_load(4, 2), Ok(1));
    let back = save(&mut *inst).unwrap().unwrap();
    assert_eq!(back, saved);
    // the form kept in a .kabl round-trips too
    assert_eq!(SavedState::from_kept(&back.to_kept()), Some(back));
}

#[test]
fn overflow_keeps_the_previous_settings_and_warns_never_truncating() {
    let mut f = Fixture::new(meta("test.over", 1, ""));
    f.save = "(i32.const -1)";
    let mut inst = instance_for(&f.wasm());
    let previous = SavedState {
        state_version: 2,
        bytes: vec![1, 2, 3, 4],
    };
    assert_eq!(save(&mut *inst), Err(StateError::Overflow { cap: 16 }));
    let (kept, warning) = capture(&mut *inst, Some(previous.clone()));
    assert_eq!(kept, Some(previous.clone()));
    assert!(warning.unwrap().contains("do not fit"));
    // a length past the buffer is just as bad
    f.save = "(i32.const 17)";
    let mut inst = instance_for(&f.wasm());
    assert!(matches!(
        save(&mut *inst),
        Err(StateError::BadLength { len: 17, cap: 16 })
    ));
    // a trap while saving keeps them too
    f.save = "unreachable";
    let mut inst = instance_for(&f.wasm());
    let (kept, warning) = capture(&mut *inst, Some(previous.clone()));
    assert_eq!(kept, Some(previous));
    assert!(warning.unwrap().contains("while saving"));
}

#[test]
fn an_older_layout_goes_through_migrate() {
    let mut f = Fixture::new(meta("test.mig", 1, ""));
    // old layout 1: the cell as four bytes; migrate adds 1000 to it
    f.migrate = Some(
        "(i32.store (i32.const 16512) (i32.add (i32.load (i32.const 16512)) (i32.const 1000))) (i32.const 4)",
    );
    let wasm = f.wasm();
    let g = group(&wasm);
    let (_, r) = g
        .restore(&SavedState {
            state_version: 1,
            bytes: 5u32.to_le_bytes().to_vec(),
        })
        .unwrap();
    assert_eq!(r, Restored::Migrated { from: 1, to: 2 });
    // what migrate hands back: the old cell plus 1000, in the transfer buffer
    let mut inst = instance_for(&wasm);
    let x = inst.state_xfer().unwrap();
    assert!(inst.has_migrate());
    inst.memory()[x.addr..x.addr + 4].copy_from_slice(&5u32.to_le_bytes());
    assert_eq!(inst.call_migrate(4, 1), Ok(4));
    assert_eq!(inst.memory()[x.addr..x.addr + 4], 1005u32.to_le_bytes());
}

#[test]
fn a_migrate_that_traps_or_refuses_falls_back_to_defaults() {
    let old = SavedState {
        state_version: 1,
        bytes: 5u32.to_le_bytes().to_vec(),
    };
    for (body, why) in [
        ("unreachable", "migrate"),
        ("(i32.const -1)", "could not convert"),
    ] {
        let mut f = Fixture::new(meta("test.migtrap", 1, ""));
        f.migrate = Some(body);
        let g = group(&f.wasm());
        let (m, r) = g.restore(&old).unwrap();
        match r {
            Restored::Defaults { why: w } => assert!(w.contains(why), "{w}"),
            other => panic!("{other:?}"),
        }
        // the lane is alive and usable
        let mut m = prepared(m);
        assert_eq!(run(&mut m, &[0.5; N]), [0.5; N]);
        assert!(!g.health().is_crashed());
    }
    // no migrate at all, and a downgrade (saved by a newer layout): defaults, never a crash
    let f = Fixture::new(meta("test.nomig", 1, ""));
    let g = group(&f.wasm());
    assert!(matches!(
        g.restore(&old).unwrap().1,
        Restored::Defaults { .. }
    ));
    let newer = SavedState {
        state_version: 3,
        bytes: vec![0; 4],
    };
    let (_, r) = g.restore(&newer).unwrap();
    assert!(
        matches!(&r, Restored::Defaults { why } if why.contains("newer")),
        "{r:?}"
    );
    // the module rejecting the bytes (load returns 0) leaves it alive at defaults
    let bad = SavedState {
        state_version: 2,
        bytes: vec![1, 2],
    };
    let (_, r) = g.restore(&bad).unwrap();
    assert!(
        matches!(&r, Restored::Defaults { why } if why.contains("did not accept")),
        "{r:?}"
    );
}

#[test]
fn the_sdk_example_saves_and_restores_its_settings() {
    let g = group(host::EXAMPLES[1].wasm);
    let c = engine().compile(host::EXAMPLES[1].wasm).unwrap();
    assert_eq!(c.meta().kind, "demo.bitfilter");
    let mut inst = instance_for(host::EXAMPLES[1].wasm);
    let saved = save(&mut *inst)
        .unwrap()
        .expect("the SDK module has settings");
    assert!(!saved.bytes.is_empty() && saved.bytes.len() <= 256);
    let (_, r) = g.restore(&saved).unwrap();
    assert_eq!(r, Restored::Loaded);
}

// ---- non-finite input

#[test]
fn non_finite_input_is_zeroed_and_counted_and_the_module_survives() {
    let f = Fixture::new(meta("test.nan", 1, ""));
    let g = group(&f.wasm());
    g.fill(1).unwrap();
    let mut m = prepared(g.voice().unwrap());
    let mut input = [0.25f32; N];
    input[3] = f32::NAN;
    input[10] = f32::INFINITY;
    input[11] = f32::NEG_INFINITY;
    let out = assert_no_alloc(|| run(&mut m, &input));
    assert!(out.iter().all(|v| v.is_finite()));
    assert_eq!((out[3], out[10], out[11], out[4]), (0.0, 0.0, 0.0, 0.25));
    assert!(
        !g.health().is_crashed(),
        "upstream garbage must not kill a pass-through"
    );
    assert_eq!(g.health().sanitized_inputs(), 3);
    // a scalar (constant) NaN input counts once per frame
    let sig = [Signal::Scalar(f32::NAN)];
    let mut out = [1f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &[], N);
        m.process(&mut io);
    }
    assert_eq!(out, [0.0; N]);
    assert_eq!(g.health().sanitized_inputs(), 3 + N as u32);
    assert!(!g.health().is_crashed());
}

#[test]
fn input_sanitising_costs_little() {
    let f = Fixture::new(meta("test.cost", 1, ""));
    let g = group(&f.wasm());
    g.fill(1).unwrap();
    let mut m = prepared(g.voice().unwrap());
    let input = [0.25f32; N];
    let t = std::time::Instant::now();
    for _ in 0..20_000 {
        run(&mut m, &input);
    }
    let per_block = t.elapsed().as_secs_f64() / 20_000.0 * 1e6;
    eprintln!("pass-through block with sanitising: {per_block:.2} us (64 frames)");
    // a 64-frame block at 48 kHz is 1333 us; stay far below any share of it
    assert!(per_block < 50.0, "{per_block} us");
}

// ---- the registry keyed by blob hash

fn versioned(kind: &str, version: u32, offset: i32) -> Vec<u8> {
    // pass-through plus a constant offset, so versions are audibly different
    let mut f = Fixture::new(meta(kind, version, ""));
    let body = format!(
        "(local $i i32) (block $d (loop $l (br_if $d (i32.ge_u (local.get $i) (local.get $n)))\
         (f32.store (i32.add (i32.const 8256) (i32.shl (local.get $i) (i32.const 2)))\
           (f32.add (f32.const {offset}.0) (f32.load (i32.add (i32.const 64) (i32.shl (local.get $i) (i32.const 2))))))\
         (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $l)))"
    );
    f.out = Box::leak(body.into_boxed_str());
    f.wasm()
}

fn render_in(scope: &CodeScope, kind: &str) -> [f32; N] {
    let inst = kabl_modules::registry::create_in(scope, kind).expect("module");
    // downcast is not needed: drive it through the Module trait
    let mut inst = inst;
    inst.prepare(
        SR,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let sig = [Signal::Buffer(&[0.5; N])];
    let mut out = [0f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &[], N);
        inst.process(&mut io);
    }
    out
}

#[test]
fn two_blobs_of_one_kind_coexist_and_a_stand_in_never_replaces_anothers_group() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let kind = "test.coexist";
    let (v1, v2) = (versioned(kind, 1, 1), versioned(kind, 2, 2));
    let (a, b) = (
        host::register_blob(&v1).unwrap(),
        host::register_blob(&v2).unwrap(),
    );
    assert_ne!(a.key, b.key);
    // song A names v1, song B names v2
    let song_a = CodeScope::new().with(kind, a.key);
    let song_b = CodeScope::new().with(kind, b.key);
    assert_eq!(render_in(&song_a, kind), [1.5; N]);
    assert_eq!(render_in(&song_b, kind), [2.5; N]);
    // song C embeds a copy the user refused: its own stand-in (pass-through), under its own key
    let refused = versioned(kind, 3, 3);
    let (_, key_c) = host::register_stand_in_keyed(kind, 3, Some(&refused), "declined", None);
    let song_c = CodeScope::new().with(kind, key_c);
    assert_eq!(
        render_in(&song_c, kind),
        [0.5; N],
        "a stand-in passes audio through"
    );
    // A and B still run their own code afterwards
    assert_eq!(render_in(&song_a, kind), [1.5; N]);
    assert_eq!(render_in(&song_b, kind), [2.5; N]);
    // each song saves its own blob back
    for (scope, bytes) in [(&song_a, &v1), (&song_b, &v2), (&song_c, &refused)] {
        let (_, got) = host::blob_for_in(scope, kind).unwrap();
        assert_eq!(&*got, &bytes[..]);
    }
    // registering one again points the unscoped lookup at it and breaks nothing
    host::register_blob(&v1).unwrap();
    assert_eq!(render_in(&CodeScope::new(), kind), [1.5; N]);
    assert_eq!(render_in(&song_b, kind), [2.5; N]);
}

#[test]
fn only_modules_in_use_divide_the_shared_budget() {
    use kabl_modules::code::{PER_MODULE_SHARE, TOTAL_SHARE};
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // Twelve revisions of one module stay registered (twelve songs were opened)...
    let regs: Vec<_> = (1..=12)
        .map(|v| host::register_blob(&versioned("test.budget", v, v as i32)).unwrap())
        .collect();
    // ...and one of them plays.
    let group = kabl_modules::code::group_by_key(&regs[3].key).unwrap();
    let voice = group.voice().unwrap();
    // The block period the host budget was built for.
    let cal = host::budget().calibration();
    let period_us = f64::from(512u32) / 48_000.0 * 1e6;
    let full = cal.fuel_for_us(period_us * PER_MODULE_SHARE);
    assert_eq!(
        group.health().fuel_budget(),
        full,
        "the module in use gets its whole share, not a twelfth of the total"
    );
    // An idle revision does not take any of the total away...
    let idle = kabl_modules::code::group_by_key(&regs[0].key).unwrap();
    assert!(idle.health().fuel_budget() > 0);
    // ...and a third and fourth module playing at once share the 50 % total as before.
    let others: Vec<_> = [5usize, 7, 9]
        .iter()
        .map(|&i| kabl_modules::code::group_by_key(&regs[i].key).unwrap())
        .collect();
    let _voices: Vec<_> = others.iter().map(|g| g.voice().unwrap()).collect();
    let quarter_of_total = cal.fuel_for_us(period_us * TOTAL_SHARE / 4.0);
    assert_eq!(group.health().fuel_budget(), quarter_of_total);
    drop(voice);
}

#[test]
fn a_module_that_cannot_make_an_instance_is_refused_at_registration() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Fixture::new(meta("test.regbad", 1, ""));
    f.xfer_cap = 65 * 1024;
    let Err(e) = host::register_blob(&f.wasm()) else {
        panic!("an over-cap settings buffer must be refused when the blob is registered");
    };
    assert!(
        matches!(&e, LoadError::Layout(m) if m.contains("cap")),
        "{e}"
    );
    assert!(host::blob_for("test.regbad").is_none(), "nothing installed");
}

#[test]
fn code_that_is_another_module_than_claimed_is_refused_and_installs_nothing() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let real = versioned("test.realkind", 4, 1);
    for (kind, version) in [("test.claimed", 4), ("test.realkind", 5)] {
        let Err(e) = host::register_blob_as(&real, kind, version) else {
            panic!("a mismatch must be refused");
        };
        assert_eq!(
            e,
            LoadError::WrongModule {
                kind: "test.realkind".into(),
                version: 4
            }
        );
        assert!(e
            .to_string()
            .contains("different module (test.realkind v4)"));
    }
    assert!(host::blob_for("test.realkind").is_none(), "not installed");
    assert!(host::register_blob_as(&real, "test.realkind", 4).is_ok());
    assert!(host::blob_for("test.realkind").is_some());
}

#[test]
fn a_failed_save_through_the_running_module_keeps_the_previous_settings_and_notes_it() {
    let mut f = Fixture::new(meta("test.cellover", 1, ""));
    f.save = "(i32.const -1)";
    let g = group(&f.wasm());
    g.fill(1).unwrap();
    let mut m = prepared(g.voice().unwrap());
    let scope = CodeScope::new();
    let previous = SavedState {
        state_version: 2,
        bytes: vec![9, 9, 9, 9],
    };
    scope.set_kept_state(7, "test.cellover", previous.clone());
    scope.attach(7, "test.cellover", &mut m);
    scope.request_capture();
    run(&mut m, &[0.0; N]); // the module serves the request at the top of the block
    assert_eq!(scope.collect(), 0);
    assert_eq!(
        scope.settings(),
        vec![(7, previous)],
        "previous settings kept"
    );
    let notes = scope.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("module 7") && n.contains("buffer")),
        "{notes:?}"
    );
    assert!(!g.health().is_crashed());
}
