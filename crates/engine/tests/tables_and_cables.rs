//! Wavetable oscillators (embedded tables, schema 5) and functional cables (schema 6, routes 7)
//! in one patch: both survive save and reload, a file master wrote at schema 5 still loads and
//! renders as before, and the pattern does not touch the table voice.

use kabl_core::{
    CableState, ModuleState, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Table, Vec2,
};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_modules::wavetable;

const SR: f32 = 48000.0;

fn module(kind: &str, kv: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: kv.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: (u64, &str), to: (u64, &str), kv: &[(&str, f32)]) -> CableState {
    let port = |(id, port): (u64, &str)| PortRef::Module {
        id,
        port: port.into(),
    };
    CableState {
        from: port(from),
        to: port(to),
        params: kv.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
        steps: Vec::new(),
    }
}

fn user_table() -> Table {
    let mut w = Vec::new();
    for k in 0..wavetable::FRAME {
        let v = (std::f64::consts::TAU * 3.0 * k as f64 / wavetable::FRAME as f64).sin();
        w.extend(((v * 30000.0) as i16).to_le_bytes());
    }
    let mut b = b"RIFF".to_vec();
    b.extend((36 + w.len() as u32).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend([1, 0, 1, 0]);
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend([2, 0, 16, 0]);
    b.extend(b"data");
    b.extend((w.len() as u32).to_le_bytes());
    b.extend(w);
    Table {
        name: "third.wav".into(),
        wav: wavetable::import_wav(&b).unwrap(),
    }
}

/// midi.in -> osc.wt (user table 1) -> out.left, plus, when `patterned`, a clock whose gate goes
/// to out.right over a cable with a three-step pattern (step 2 silent, step 3 at 60 %).
fn patch(patterned: bool) -> PatchState {
    let mut p = PatchState::new();
    p.tables.insert(1, user_table());
    p.modules.insert(1, module("midi.in", &[]));
    p.modules
        .insert(2, module("osc.wt", &[("table", 5.0), ("user", 1.0)]));
    p.modules.insert(4, module("out", &[]));
    p.cables.insert(1, cable((1, "pitch"), (2, "pitch"), &[]));
    p.cables.insert(2, cable((2, "out"), (4, "left"), &[]));
    if patterned {
        p.modules.insert(7, module("clock", &[("bpm", 300.0)]));
        let pattern = [("length", 3.0), ("s2", 0.0), ("r3", 60.0)];
        p.cables
            .insert(3, cable((7, "gate"), (4, "right"), &pattern));
    }
    p
}

fn log_of(p: &PatchState) -> PatchLog {
    let mut log = PatchLog::new();
    for (&slot, t) in &p.tables {
        let op = Op::SetTable {
            slot,
            value: Some(t.clone()),
        };
        log.append(op, 0, Source::User);
    }
    for (&id, m) in &p.modules {
        let op = Op::AddModule {
            id,
            kind: m.kind.clone(),
            pos: m.pos,
        };
        log.append(op, 0, Source::User);
        for (name, &value) in &m.params {
            let target = ParamTarget::Module {
                id,
                param: name.clone(),
            };
            log.append(Op::SetParam { target, value }, 0, Source::User);
        }
    }
    for (&id, c) in &p.cables {
        let op = Op::Connect {
            id,
            from: c.from.clone(),
            to: c.to.clone(),
        };
        log.append(op, 0, Source::User);
        for (name, &value) in &c.params {
            let target = ParamTarget::Cable {
                id,
                param: name.clone(),
            };
            log.append(Op::SetParam { target, value }, 0, Source::User);
        }
    }
    log
}

fn render(c: &mut CompiledPatch, blocks: usize) -> (Vec<f32>, Vec<f32>) {
    c.note_on(0, 2.0, 0.8);
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for _ in 0..blocks {
        c.process_block();
        l.extend_from_slice(c.left());
        r.extend_from_slice(c.right());
    }
    (l, r)
}

fn render_state(p: &PatchState) -> (Vec<f32>, Vec<f32>) {
    render(&mut compile(p, SR, 2).unwrap(), 200)
}

#[test]
fn a_table_voice_and_a_patterned_cable_play_together_and_survive_save_and_reload() {
    let plain = render_state(&patch(false));
    let both = render_state(&patch(true));
    assert!(
        plain.0.iter().any(|s| s.abs() > 0.01),
        "the table voice sounds"
    );
    assert_eq!(
        both.0, plain.0,
        "the pattern does not touch the table voice"
    );
    assert!(
        both.1.iter().any(|s| s.abs() > 0.01),
        "the patterned clock sounds"
    );
    assert_ne!(
        both.1,
        render_state(&{
            let mut p = patch(true);
            p.cables.get_mut(&3).unwrap().params.clear();
            p
        })
        .1,
        "the pattern changes the clock cable"
    );

    let log = log_of(&patch(true));
    assert_eq!(log.state(), &patch(true));
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    let back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(back.state(), log.state());
    assert_eq!(render_state(back.state()), both);
}

#[test]
fn a_file_master_wrote_at_schema_5_with_a_table_loads_and_renders_unchanged() {
    // Schema 5 on master means tables and nothing else: the same log, `meta.toml` says 5.
    let log = log_of(&patch(false));
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    assert!(meta.contains(&format!(
        "schema_version = {}",
        kabl_core::CURRENT_SCHEMA_VERSION
    )));
    std::fs::write(
        dir.path().join("meta.toml"),
        meta.replace(
            &format!("schema_version = {}", kabl_core::CURRENT_SCHEMA_VERSION),
            "schema_version = 5",
        ),
    )
    .unwrap();
    let back = kabl_core::load(dir.path()).expect("a schema-5 file loads");
    assert_eq!(back.state().tables, log.state().tables);
    assert_eq!(render_state(back.state()), render_state(&patch(false)));
}

#[test]
fn a_file_with_a_table_is_refused_below_schema_5() {
    let log = log_of(&patch(false));
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    std::fs::write(dir.path().join("meta.toml"), "schema_version = 4\n").unwrap();
    assert!(kabl_core::load(dir.path()).is_err());
}
