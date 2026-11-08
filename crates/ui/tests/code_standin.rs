//! D11: a patch whose code module cannot load still opens. Each failed module becomes a stand-in
//! (same ports and params, audio passes through), the user gets a warning naming module id,
//! version and cause, and saving writes back the original blob and settings unchanged.

use std::collections::BTreeMap;
use std::path::PathBuf;

use basedrop::Collector;
use kabl_core::embedded::{hash_hex, hash_of, Embedded};
use kabl_core::{format, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_ui::{code_notice, library};

fn wasm(kind: &str, abi_line: &str, imports: &str) -> Vec<u8> {
    let meta = format!(
        "kind {kind}\\nversion 1\\nname T\\nexplain Test fixture.\\ncategory filter\\nrate voice\\nwidth 4\\n\
{abi_line}\\nport in audio in 1\\nport out audio out 1\\nparam gain 0 4 1 linear 0 x\\n"
    );
    wat::parse_str(format!(
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
 (func (export "process") (param $n i32)))"#
    ))
    .unwrap()
}

fn port(id: u64, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

/// midi.in -> osc.va -> [code module 10, gain 1.5] -> out. `None` kind: osc straight to out.
fn patch(kind: Option<&str>) -> PatchLog {
    let mut log = PatchLog::new();
    let mut t = 0;
    let mut op = |log: &mut PatchLog, o: Op| {
        t += 1;
        log.append(o, t, Source::User)
    };
    let mut add = |log: &mut PatchLog, id, kind: &str| {
        op(
            log,
            Op::AddModule {
                id,
                kind: kind.into(),
                pos: Vec2 { x: 0.0, y: 0.0 },
            },
        )
    };
    add(&mut log, 1, "midi.in");
    add(&mut log, 2, "osc.va");
    add(&mut log, 4, "out");
    let mut cables = vec![(1, "pitch", 2, "pitch")];
    match kind {
        Some(k) => {
            add(&mut log, 10, k);
            op(
                &mut log,
                Op::SetParam {
                    target: ParamTarget::Module {
                        id: 10,
                        param: "gain".into(),
                    },
                    value: 1.5,
                },
            );
            cables.extend([(2, "out", 10, "in"), (10, "out", 4, "in")]);
        }
        None => cables.push((2, "out", 4, "in")),
    }
    for (i, (f, fp, to, tp)) in cables.into_iter().enumerate() {
        op(
            &mut log,
            Op::Connect {
                id: i as u64 + 1,
                from: port(f, fp),
                to: port(to, tp),
            },
        );
    }
    log
}

fn file(name: &str, log: &PatchLog, code: &Embedded) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kabl-standin-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.kabl");
    format::save_kabl_full(&path, log, None, Some(code)).unwrap();
    path
}

fn with_blob(kind: &str, bytes: &[u8]) -> Embedded {
    let mut e = Embedded::new();
    let h = e.add_blob(kind, 1, bytes.to_vec()).unwrap();
    e.bind(10, h).unwrap();
    e
}

fn render(state: &PatchState) -> Vec<u32> {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, state, 48000.0, 4).unwrap();
    e.key(KeyEvent::On {
        note: 57,
        velocity: 100,
    });
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut out = Vec::new();
    for _ in 0..100 {
        e.process_block(&mut l, &mut r);
        out.extend(l.iter().map(|v| v.to_bits()));
    }
    out
}

/// Opens `path` the way the library does and returns the log and the warnings.
fn open(path: &std::path::Path, trust: &BTreeMap<String, bool>) -> (PatchLog, Vec<String>) {
    let log = library::read_patch(path, trust).expect("the patch must open with a stand-in");
    let w = code_notice::warnings(log.state());
    (log, w)
}

#[test]
fn every_failure_cause_opens_the_patch_with_a_stand_in_and_a_warning() {
    let none = BTreeMap::new();
    let mut declined = BTreeMap::new();
    let good = wasm("test.si.declined", "", "");
    declined.insert(hash_hex(&hash_of(&good)), false);

    // (kind, blob bytes or None, trust, text the warning must contain)
    type Case<'a> = (
        &'a str,
        Option<Vec<u8>>,
        &'a BTreeMap<String, bool>,
        &'a str,
    );
    let cases: Vec<Case> = vec![
        ("test.si.declined", Some(good), &declined, "declined"),
        (
            "test.si.invalid",
            Some(b"\0asm\x01\0\0\0junk".to_vec()),
            &none,
            "invalid",
        ),
        (
            "test.si.abi",
            Some(wasm("test.si.abi", "abi 7", "")),
            &none,
            "abi",
        ),
        (
            "test.si.import",
            Some(wasm(
                "test.si.import",
                "",
                r#"(import "env" "evil" (func))"#,
            )),
            &none,
            "env.evil",
        ),
        ("test.si.missing", None, &none, "not in this file"),
    ];
    for (kind, bytes, trust, cause) in cases {
        let mut code = Embedded::new();
        if let Some(b) = &bytes {
            code = with_blob(kind, b);
        }
        // Trust for the non-declined cases: the notice was answered "open".
        let mut trust = trust.clone();
        if let Some(b) = &bytes {
            trust.entry(hash_hex(&hash_of(b))).or_insert(true);
        }
        let path = file(kind, &patch(Some(kind)), &code);
        let (log, w) = open(&path, &trust);
        assert_eq!(w.len(), 1, "{kind}: {w:?}");
        assert!(w[0].contains(kind) && w[0].contains("v1") || kind == "test.si.missing");
        assert!(
            w[0].to_lowercase().contains(cause),
            "{kind}: warning {:?} lacks {cause:?}",
            w[0]
        );
        // The settings are untouched.
        assert_eq!(log.state().modules[&10].params["gain"], 1.5, "{kind}");
        assert_eq!(log.state().modules[&10].kind, kind);
    }
}

#[test]
fn a_stand_in_passes_audio_through_bit_exact() {
    let bytes = wasm("test.si.pass", "abi 7", "");
    let path = file(
        "pass",
        &patch(Some("test.si.pass")),
        &with_blob("test.si.pass", &bytes),
    );
    let trust = BTreeMap::from([(hash_hex(&hash_of(&bytes)), true)]);
    let (log, _) = open(&path, &trust);
    let through = render(log.state());
    let direct = render(patch(None).state());
    assert!(through.iter().any(|b| f32::from_bits(*b).abs() > 0.01));
    assert_eq!(through, direct, "pass-through must be bit-exact");
}

#[test]
fn saving_through_a_stand_in_keeps_the_blob_and_settings() {
    let bytes = wasm("test.si.keep", "", r#"(import "env" "evil" (func))"#);
    let original = patch(Some("test.si.keep"));
    let path = file("keep", &original, &with_blob("test.si.keep", &bytes));
    let trust = BTreeMap::from([(hash_hex(&hash_of(&bytes)), true)]);
    let (log, w) = open(&path, &trust);
    assert_eq!(w.len(), 1);

    // Save again exactly as the app does.
    let code = code_notice::embedded_for(log.state());
    assert_eq!(code.instance_blob(10).unwrap().bytes, bytes);
    assert_eq!(code.instance_blob(10).unwrap().version, 1);
    let again = file("keep2", &log, &code);
    let (back, _, back_code) = format::load_kabl_full(&again).unwrap();
    assert_eq!(back.state(), original.state(), "settings and params kept");
    assert_eq!(back_code.instance_hash(10), Some(&hash_of(&bytes)));
    assert_eq!(back_code.instance_blob(10).unwrap().bytes, bytes);
}
