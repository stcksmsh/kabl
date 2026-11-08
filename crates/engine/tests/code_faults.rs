//! D11 PR 6: code modules inside a real patch. Each fault keeps the engine playing and leaves the
//! right warning on the module's health; a patch saved with embedded code reopens bit-exact;
//! 16 voices x 4 code modules allocate nothing on the audio thread.
//!
//! The wasm modules here are test-only fixtures: a gain stage that misbehaves only when its
//! `gain` parameter is above 2 (the load-time priming block runs with 0, so loading succeeds).

use std::sync::Arc;
use std::time::Duration;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::embedded::{hash_of, Embedded};
use kabl_core::format;
use kabl_core::{Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::code::{
    self, CodeGroup, Fault, InstanceConfig, LoadError, WasmEngine, WasmiEngine,
};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;

/// `abi_line` is empty or `abi N`.
fn wasm(kind: &str, abi_line: &str, imports: &str, bad: &str) -> Vec<u8> {
    let meta = format!(
        "kind {kind}\\nversion 1\\nname T\\nexplain Test fixture.\\ncategory filter\\nrate voice\\nwidth 4\\n\
{abi_line}\\nport in audio in 1\\nport out audio out 1\\nparam gain 0 4 1 linear 0 x\\n"
    );
    let wat = format!(
        r#"(module {imports}
 (@custom "kabl.meta" "{meta}")
 (memory (export "memory") 1)
 (func (export "init") (param f32 i32) (result i32)
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 320))
  (i32.store (i32.const 8) (i32.const 576)) (i32.store (i32.const 12) (i32.const 600))
  (i32.store (i32.const 16) (i32.const 12)) (i32.store (i32.const 20) (i32.const 64))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 1))
  (i32.const 0))
 (func (export "reset"))
 (func (export "process") (param $n i32) (local $i i32)
  (block $done (loop $l
   (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
   (f32.store (i32.add (i32.const 320) (i32.shl (local.get $i) (i32.const 2)))
    (f32.mul (f32.load (i32.add (i32.const 64) (i32.shl (local.get $i) (i32.const 2))))
             (f32.load (i32.const 576))))
   (local.set $i (i32.add (local.get $i) (i32.const 1)))
   (br $l)))
  (if (f32.gt (f32.load (i32.const 576)) (f32.const 2.0)) (then {bad}))))"#
    );
    wat::parse_str(wat).unwrap()
}

fn register(bytes: &[u8], voices: usize) -> Result<Arc<CodeGroup>, LoadError> {
    let compiled = WasmiEngine::new().compile(bytes)?;
    let g = CodeGroup::new(
        compiled,
        InstanceConfig::new(SR, BLOCK as u32),
        2_000_000,
        Duration::from_secs(1),
    )?;
    g.fill(voices)?;
    code::register(g.clone())?;
    Ok(g)
}

fn cable(from: u64, fp: &str, to: u64, tp: &str) -> (PortRef, PortRef) {
    let p = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    (p(from, fp), p(to, tp))
}

/// midi.in -> osc.va -> chain of code modules -> out, plus a second oscillator straight to out
/// (the part of the synth that must keep playing). Code module ids are 10, 11, ...
fn patch(kinds: &[&str], gain: f32) -> PatchLog {
    let mut log = PatchLog::new();
    let mut t = 0;
    let mut op = |log: &mut PatchLog, o: Op| {
        t += 1;
        log.append(o, t, Source::User)
    };
    let add = |log: &mut PatchLog, op: &mut dyn FnMut(&mut PatchLog, Op), id, kind: &str| {
        op(
            log,
            Op::AddModule {
                id,
                kind: kind.into(),
                pos: Vec2 { x: 0.0, y: 0.0 },
            },
        )
    };
    add(&mut log, &mut op, 1, "midi.in");
    add(&mut log, &mut op, 2, "osc.va");
    add(&mut log, &mut op, 4, "out");
    add(&mut log, &mut op, 5, "osc.va");
    let mut cid = 1;
    let mut wire = |log: &mut PatchLog, op: &mut dyn FnMut(&mut PatchLog, Op), f, fp, to, tp| {
        let (from, to) = cable(f, fp, to, tp);
        op(log, Op::Connect { id: cid, from, to });
        cid += 1;
    };
    wire(&mut log, &mut op, 1, "pitch", 2, "pitch");
    wire(&mut log, &mut op, 1, "pitch", 5, "pitch");
    wire(&mut log, &mut op, 5, "out", 4, "in");
    let mut prev = 2;
    for (i, k) in kinds.iter().enumerate() {
        let id = 10 + i as u64;
        add(&mut log, &mut op, id, k);
        op(
            &mut log,
            Op::SetParam {
                target: ParamTarget::Module {
                    id,
                    param: "gain".into(),
                },
                value: gain,
            },
        );
        wire(&mut log, &mut op, prev, "out", id, "in");
        prev = id;
    }
    wire(&mut log, &mut op, prev, "out", 4, "in");
    log
}

fn render(
    state: &PatchState,
    voices: usize,
    blocks: usize,
    hold: &[u8],
) -> (Vec<f32>, PatchEngine) {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, state, SR, voices).unwrap();
    for &n in hold {
        e.key(KeyEvent::On {
            note: n,
            velocity: 100,
        });
    }
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut out = Vec::new();
    for _ in 0..blocks {
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    (out, e)
}

fn tail_peak(x: &[f32]) -> f32 {
    x[x.len() - BLOCK * 8..]
        .iter()
        .fold(0.0, |a, v| f32::max(a, v.abs()))
}

#[test]
fn every_runtime_fault_keeps_the_synth_playing_and_flags_the_module() {
    let cases = [
        ("test.f.trap", "unreachable", Fault::Trap),
        ("test.f.spin", "(loop $s (br $s))", Fault::Overrun),
        (
            "test.f.grow",
            "(drop (memory.grow (i32.const 1)))",
            Fault::MemoryOverCap,
        ),
        (
            "test.f.nan",
            "(f32.store (i32.const 320) (f32.const nan))",
            Fault::NonFinite,
        ),
    ];
    for (kind, bad, want) in cases {
        let g = register(&wasm(kind, "", "", bad), 4).unwrap();
        let log = patch(&[kind], 3.0);
        let (out, _) = render(log.state(), 4, 120, &[57]);
        assert_eq!(g.health().fault(), Some(want), "{kind}");
        assert!(g.health().take_notice(), "{kind}: toast raised once");
        assert!(!g.health().take_notice(), "{kind}: only once");
        assert!(out.iter().all(|v| v.is_finite()), "{kind}: finite output");
        assert!(tail_peak(&out) > 0.01, "{kind}: the other oscillator plays");
        assert!(!want.label().is_empty());
        // A fresh gain below the threshold would run fine: the module stays off until reset.
        g.health().clear();
        assert!(!g.health().is_crashed());
        code::unregister(kind);
    }
}

#[test]
fn load_time_faults_are_refused_with_a_message_and_never_register() {
    let eng = WasmiEngine::new();
    let err = |b: Vec<u8>| eng.compile(&b).err().expect("must be refused");
    let e = err(wasm(
        "test.l.imp",
        "",
        r#"(import "env" "evil" (func))"#,
        "nop",
    ));
    assert!(matches!(e, LoadError::ForbiddenImport(_)), "{e}");
    assert!(e.to_string().contains("env.evil"));
    let e = err(wasm("test.l.abi", "abi 1", "", "nop"));
    assert_eq!(e, LoadError::AbiMismatch { module: 1, host: 0 });
    assert!(e.to_string().to_lowercase().contains("abi"));
    let e = err(vec![0; code::MAX_BLOB_BYTES + 1]);
    assert!(matches!(e, LoadError::TooLarge { .. }), "{e}");
    // The same ABI line at the host's own version is accepted.
    assert!(eng.compile(&wasm("test.l.ok", "abi 0", "", "nop")).is_ok());
    assert!(code::group("test.l.imp").is_none());
}

#[test]
fn a_patch_saved_with_code_reopens_bit_exact() {
    let kind = "test.r.gain";
    let bytes = wasm(kind, "", "", "nop");
    let log = patch(&[kind, kind], 1.5);
    let g = register(&bytes, 4).unwrap();
    let (before, _) = render(log.state(), 4, 200, &[52, 59]);
    assert!(tail_peak(&before) > 0.01);

    let mut emb = Embedded::new();
    let h = emb.add_blob(kind, 1, bytes.clone()).unwrap();
    emb.bind(10, h).unwrap();
    emb.bind(11, h).unwrap();
    let mut file = Vec::new();
    format::write_kabl_full(&mut file, &log, None, Some(&emb)).unwrap();

    // Reopen: the module is gone from the registry until the embedded copy is loaded again.
    drop(g);
    code::unregister(kind);
    let (back, _, emb2) = format::read_kabl_full(&file[..]).unwrap();
    let blob = emb2.instance_blob(10).unwrap();
    assert_eq!(blob.module_id, kind);
    assert_eq!(emb2.instance_hash(10), Some(&hash_of(&bytes)));
    register(&blob.bytes, 4).unwrap();
    let (after, _) = render(back.state(), 4, 200, &[52, 59]);
    assert_eq!(
        before.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        after.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "audio differs after save and reopen"
    );
    code::unregister(kind);
}

#[test]
fn sixteen_voices_times_four_code_modules_allocate_nothing() {
    let kinds = ["test.a.0", "test.a.1", "test.a.2", "test.a.3"];
    for k in kinds {
        register(&wasm(k, "", "", "nop"), 16).unwrap();
    }
    let log = patch(&kinds, 1.0);
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, log.state(), SR, 16).unwrap();
    let chord: Vec<u8> = (40..56).collect();
    for &n in &chord {
        e.key(KeyEvent::On {
            note: n,
            velocity: 100,
        });
    }
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    // Warm up outside the check (first-block setup is not the audio path), then watch.
    for _ in 0..20 {
        e.process_block(&mut l, &mut r);
    }
    let mut peak = 0.0f32;
    for _ in 0..300 {
        assert_no_alloc(|| e.process_block(&mut l, &mut r));
        peak = l.iter().fold(peak, |a, v| a.max(v.abs()));
    }
    assert!(peak > 0.01);
    for k in kinds {
        let g = code::group(k).unwrap();
        assert!(!g.health().is_crashed(), "{k}");
        assert_eq!(g.health().voices(), 16, "{k}");
        code::unregister(k);
    }
}
