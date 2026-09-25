//! The sound library (`kabl_ui::library`): factory discovery and metadata, user Save As /
//! Save / rename, name and directory collisions, factory protection, read-only and failed
//! saves, interrupted-save repair, malformed and uncompilable patches, favorites and recents
//! (missing entries included), old patches without metadata.
//!
//! `write_factory_library` (ignored) writes the init keyboard patch and every factory
//! `sound.toml`: `cargo test -p kabl-ui --test library write_factory_library -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_ui::library::{self, LibError, Library, Meta, Origin, CATEGORIES};
use kabl_ui::perform::PIN_PREFIX;
use kabl_ui::PatchEditor;

fn factory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .canonicalize()
        .unwrap()
}

/// Every factory sound: (dir, name, category, tags, description).
const FACTORY: &[(&str, &str, &str, &[&str], &str)] = &[
    (
        "init-keyboard",
        "Init Keyboard",
        "Basic",
        &["starting point", "simple", "saw"],
        "A plain keyboard voice to start from: one saw oscillator, a low-pass filter and an \
         envelope. Its main controls are on the Perform panel.",
    ),
    (
        "palette/strings",
        "Ensemble Strings",
        "Strings",
        &["warm", "chorus", "poly", "evolving"],
        "Animated ensemble strings: two detuned saws, slow bow envelope, vibrato, stereo chorus.",
    ),
    (
        "palette/pad",
        "Evolving Pad",
        "Pad",
        &["warm", "slow", "poly", "evolving", "echo"],
        "Warm evolving pad: slow pulse-width motion, a breathing driven ladder filter, echo and \
         a big room.",
    ),
    (
        "palette/lead",
        "Singing Lead",
        "Lead",
        &["mono", "legato", "glide", "bright"],
        "Singing mono lead: legato glide between overlapping notes, a synced vowel on each \
         attack, drive and ping-pong echo.",
    ),
    (
        "palette/bass",
        "Sequence Bass",
        "Bass",
        &["sequenced", "resonant", "dry"],
        "Resonant sequence bass: saw plus square sub through a resonant ladder, accents from \
         the sequencer. Runs its own sequence; press Start.",
    ),
    (
        "palette/breath",
        "Breath",
        "Wind",
        &["noise", "airy", "poly", "texture"],
        "Airy wind texture: noise through a narrow band that follows the key, gusts from a slow \
         LFO, wide chorus and a long room.",
    ),
    (
        "palette/perc",
        "Noise Percussion",
        "Percussion",
        &["drums", "sequenced", "noise"],
        "Kick, snare and hat from noise and sines on three sequencers from one clock. Runs its \
         own sequence; press Start.",
    ),
    (
        "sound-palette",
        "Sound Palette Piece",
        "Piece",
        &["sequenced", "layers", "cues", "keyboard"],
        "The palette voices together: sequenced bass and percussion with section cues, and \
         four keyboard layers on faders.",
    ),
    (
        "composition",
        "Composition",
        "Piece",
        &["sequenced", "banks", "cues", "evolving", "keyboard"],
        "Interlocking sequences that move through sections: pattern banks, cues, a synced LFO \
         and macros.",
    ),
    (
        "performance",
        "Performance",
        "Piece",
        &["sequenced", "reverb", "echo", "keyboard"],
        "A sequenced piece with a plate reverb, echo, velocity and gate length, set up for the \
         Perform panel.",
    ),
    (
        "interlocking",
        "Interlocking Sequences",
        "Piece",
        &["sequenced", "polyrhythm"],
        "A 7-step bass on 16ths against a 5-step lead on 8ths from one clock.",
    ),
    (
        "echo",
        "Echo Sequence",
        "Piece",
        &["sequenced", "echo", "delay"],
        "A sequence through the clock-synced echo.",
    ),
    (
        "sequence",
        "First Sequence",
        "Piece",
        &["sequenced", "simple"],
        "The smallest sequence: one clock and one 8-step sequencer playing a voice.",
    ),
    (
        "reference",
        "Reference Voice",
        "Study",
        &["keyboard", "modulation", "lfo"],
        "The rack's reference patch: a keyboard voice with LFO and envelope routes into the \
         filter and envelope.",
    ),
    (
        "crowded",
        "Crowded Rack",
        "Study",
        &["keyboard", "layout"],
        "Many modules and cables, for trying the rack's cable views and zoom.",
    ),
];

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

/// The initialized keyboard patch: midi.in → saw → low-pass → VCA (ADSR) → out, with its
/// five main controls pinned to the Perform panel.
fn init_keyboard() -> PatchEditor {
    let mut e = PatchEditor::new();
    let at = |row: usize, x: f32| Vec2 {
        x,
        y: kabl_ui::rack::row_y(row),
    };
    let midi = e.add_module("midi.in", at(0, 24.0));
    let osc = e.add_module("osc.va", at(0, 240.0));
    let filter = e.add_module("filter.svf", at(0, 420.0));
    let vca = e.add_module("vca", at(0, 600.0));
    let out = e.add_module("out", at(0, 760.0));
    let env = e.add_module("env.adsr", at(1, 600.0));
    e.set_param(osc, "waveform", 2.0); // saw
    e.set_param(filter, "cutoff_hz", 2200.0);
    e.set_param(filter, "resonance", 0.2);
    e.set_param(env, "attack_ms", 8.0);
    e.set_param(env, "decay_ms", 300.0);
    e.set_param(env, "sustain", 0.7);
    e.set_param(env, "release_ms", 350.0);
    e.set_param(vca, "gain", 0.0);
    e.connect(port(midi, "pitch"), port(osc, "pitch"));
    e.connect(port(osc, "out"), port(filter, "in"));
    e.connect(port(filter, "lp"), port(vca, "in"));
    e.connect(port(midi, "gate"), port(env, "gate"));
    e.connect(port(env, "out"), port(vca, "cv"));
    e.connect(port(vca, "out"), port(out, "left"));
    e.connect(port(vca, "out"), port(out, "right"));
    let pins: [(ModuleId, &str, &str); 5] = [
        (filter, "cutoff_hz", "Brightness"),
        (filter, "resonance", "Resonance"),
        (env, "attack_ms", "Attack"),
        (env, "release_ms", "Release"),
        (osc, "waveform", "Wave"),
    ];
    let changes: Vec<_> = pins
        .iter()
        .enumerate()
        .map(|(i, (id, key, _))| (*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)))
        .collect();
    e.set_presentation(&changes);
    for (id, key, label) in pins {
        e.set_label(id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
    }
    e
}

#[test]
#[ignore = "writes patches/init-keyboard and every factory sound.toml"]
fn write_factory_library() {
    let root = factory();
    kabl_core::save(&root.join("init-keyboard"), init_keyboard().log()).unwrap();
    for (dir, name, category, tags, description) in FACTORY {
        let log = kabl_core::load(&root.join(dir)).unwrap();
        let (keys, sequence) = Meta::play_of(log.state());
        let meta = Meta {
            name: name.to_string(),
            category: category.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            description: description.split_whitespace().collect::<Vec<_>>().join(" "),
            keys,
            sequence,
        };
        fs::write(
            root.join(dir).join(library::META_FILE),
            toml::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
    }
}

fn open(user: &Path) -> Library {
    Library::open(Some(factory()), user.to_path_buf())
}

#[test]
fn the_init_keyboard_file_is_what_the_code_builds() {
    let saved = kabl_core::load(&factory().join("init-keyboard")).unwrap();
    assert_eq!(saved.state(), init_keyboard().state());
}

#[test]
fn every_factory_sound_is_discoverable_categorized_and_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    assert!(lib.notes.is_empty(), "{:?}", lib.notes);
    let factory: Vec<_> = lib
        .entries
        .iter()
        .filter(|e| e.origin == Origin::Factory)
        .collect();
    assert_eq!(
        factory.len(),
        FACTORY.len(),
        "no stray or missing factory sound"
    );
    for (dir, name, category, _, _) in FACTORY {
        let e = lib
            .get(&format!("factory:{dir}"))
            .unwrap_or_else(|| panic!("{dir} discovered"));
        assert_eq!(e.meta.name, *name);
        assert_eq!(e.meta.category, *category);
        assert!(CATEGORIES.contains(category));
        let log = lib.read(&e.id).unwrap_or_else(|err| panic!("{dir}: {err}"));
        assert_eq!(
            (e.meta.keys, e.meta.sequence),
            Meta::play_of(log.state()),
            "{dir}: the saved play flags describe the patch"
        );
    }
    // Voices vs pieces, as the browser shows them.
    let keys_only = |id: &str| {
        let m = &lib.get(id).unwrap().meta;
        m.keys && !m.sequence
    };
    for v in ["strings", "pad", "lead", "breath"] {
        assert!(
            keys_only(&format!("factory:palette/{v}")),
            "{v} is a keyboard voice"
        );
    }
    assert!(keys_only("factory:init-keyboard"));
    for s in [
        "palette/bass",
        "palette/perc",
        "sequence",
        "interlocking",
        "echo",
    ] {
        let m = &lib.get(&format!("factory:{s}")).unwrap().meta;
        assert!(m.sequence && !m.keys, "{s} runs a sequence");
    }
    let m = &lib.get("factory:composition").unwrap().meta;
    assert!(m.sequence && m.keys, "a piece you can also play");
}

#[test]
fn search_matches_every_word_across_name_category_and_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    let hits = |q: &str| -> Vec<String> {
        lib.entries
            .iter()
            .filter(|e| e.matches(q))
            .map(|e| e.id.clone())
            .collect()
    };
    assert_eq!(hits("singing"), ["factory:palette/lead"]);
    assert_eq!(hits("PAD warm"), ["factory:palette/pad"]);
    assert!(hits("sequenced").contains(&"factory:palette/bass".to_string()));
    assert!(hits("no such sound").is_empty());
    assert_eq!(hits("").len(), FACTORY.len());
}

fn edited_pad(lib: &Library) -> PatchEditor {
    let mut e = PatchEditor::from_log(lib.read("factory:palette/pad").unwrap());
    let (&id, _) = e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "filter.ladder")
        .unwrap();
    e.set_param(id, "cutoff_hz", 3333.0);
    e
}

fn dir_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn save_as_reopen_rename_and_the_factory_file_is_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let before = dir_bytes(&factory().join("palette/pad"));
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "My Pad".into(),
        category: "Pad".into(),
        tags: vec!["mine".into()],
        ..Meta::default()
    };
    assert!(matches!(
        lib.save(e.log(), meta.clone(), Some("factory:palette/pad")),
        Err(LibError::Factory)
    ));
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    assert_eq!(id, "user:my-pad");
    assert_eq!(dir_bytes(&factory().join("palette/pad")), before);

    // Found again after a restart, with its edit and full history.
    let mut lib = open(tmp.path());
    let saved = lib.get(&id).unwrap().clone();
    assert_eq!(saved.origin, Origin::User);
    assert_eq!(saved.meta.name, "My Pad");
    assert!(saved.meta.keys && !saved.meta.sequence);
    assert_eq!(
        lib.prefs.recents.first().map(String::as_str),
        Some(id.as_str())
    );
    let back = lib.read(&id).unwrap();
    assert_eq!(back.state(), e.state());
    assert_eq!(
        back.entries().len(),
        e.log().entries().len(),
        "undo history kept"
    );
    assert!(lib.entries.iter().any(|x| x.matches("my pad mine")));

    // Rename keeps the identity (and so favorites/recents); the directory stays.
    lib.toggle_favorite(&id);
    lib.set_meta(
        &id,
        Meta {
            name: "Brighter Pad".into(),
            ..saved.meta.clone()
        },
    )
    .unwrap();
    let lib = open(tmp.path());
    assert_eq!(lib.get(&id).unwrap().meta.name, "Brighter Pad");
    assert!(lib.prefs.favorites.contains(&id));
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn names_and_directories_never_collide_silently() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let named = |n: &str| Meta {
        name: n.into(),
        ..Meta::default()
    };
    let a = lib.save(e.log(), named("Pad One"), None).unwrap();
    // Same name, other case: refused, naming the sound that has it.
    match lib.save(e.log(), named(" pad one "), None) {
        Err(LibError::NameTaken(id)) => assert_eq!(id, a),
        other => panic!("{other:?}"),
    }
    // Replacing it is explicit.
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    assert_eq!(lib.save(e2.log(), named("Pad One"), Some(&a)).unwrap(), a);
    assert_eq!(lib.read(&a).unwrap().state(), e2.state());
    // A different name with the same directory slug gets its own directory.
    let b = lib.save(e.log(), named("Pad-One!"), None).unwrap();
    assert_eq!(b, "user:pad-one-2");
    assert_eq!(lib.read(&a).unwrap().state(), e2.state(), "a untouched");
    // Rename onto a taken name is refused and changes nothing.
    assert!(matches!(
        lib.set_meta(&b, named("PAD ONE")),
        Err(LibError::NameTaken(_))
    ));
    assert_eq!(lib.get(&b).unwrap().meta.name, "Pad-One!");
    // Factory names don't block user names: identity is not the name.
    lib.save(e.log(), named("Evolving Pad"), None).unwrap();
    assert!(matches!(
        lib.save(e.log(), named("   "), None),
        Err(LibError::BadName(_))
    ));
    assert!(matches!(
        lib.set_meta("factory:palette/pad", named("x")),
        Err(LibError::Factory)
    ));
}

#[test]
fn a_failed_save_leaves_the_old_sound_and_explains() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let dir = lib.get(&id).unwrap().dir.clone();
    let before = dir_bytes(&dir);
    // The staging directory's place is taken by a file: the save fails before any swap.
    fs::write(tmp.path().join("sounds/.keep.new"), b"in the way").unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    let err = lib.save(e2.log(), meta, Some(&id)).unwrap_err();
    assert!(matches!(err, LibError::Io(_)), "{err:?}");
    assert!(!err.to_string().is_empty());
    assert_eq!(dir_bytes(&dir), before, "the saved sound is intact");
}

#[cfg(unix)]
#[test]
fn a_read_only_user_directory_is_an_error_not_a_panic() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ro");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores permissions; a user root that is a file fails the same way for everyone.
    let root = if fs::write(root.join("probe"), b"").is_ok() {
        let file = tmp.path().join("a-file");
        fs::write(&file, b"").unwrap();
        file
    } else {
        root
    };
    let mut lib = open(&root);
    let e = edited_pad(&lib);
    let err = lib
        .save(
            e.log(),
            Meta {
                name: "Nope".into(),
                ..Meta::default()
            },
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("can't create"), "{err}");
    assert!(
        lib.toggle_favorite("factory:palette/pad").is_some(),
        "reported"
    );
    assert!(
        lib.prefs.favorites.contains("factory:palette/pad"),
        "kept for the session"
    );
}

#[test]
fn an_interrupted_save_is_repaired_on_the_next_scan() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let id = lib
        .save(
            e.log(),
            Meta {
                name: "Swap".into(),
                ..Meta::default()
            },
            None,
        )
        .unwrap();
    let sounds = tmp.path().join("sounds");
    // Stopped after "target → .old", before ".new → target": the old one comes back.
    fs::rename(sounds.join("swap"), sounds.join(".swap.old")).unwrap();
    fs::create_dir_all(sounds.join(".swap.new")).unwrap();
    let lib = open(tmp.path());
    assert!(
        lib.notes.iter().any(|n| n.contains("restored")),
        "{:?}",
        lib.notes
    );
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
    assert!(!sounds.join(".swap.new").exists() && !sounds.join(".swap.old").exists());
    // Stopped after ".new → target": the leftover .old is removed.
    fs::create_dir_all(sounds.join(".swap.old")).unwrap();
    let lib = open(tmp.path());
    assert!(!sounds.join(".swap.old").exists());
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn malformed_and_uncompilable_patches_are_reported_not_loaded() {
    let tmp = tempfile::tempdir().unwrap();
    let sounds = tmp.path().join("sounds");
    // A copied-in patch whose log is cut short.
    let bad = sounds.join("broken");
    fs::create_dir_all(&bad).unwrap();
    fs::copy(factory().join("reference/meta.toml"), bad.join("meta.toml")).unwrap();
    fs::write(
        bad.join("log.jsonl"),
        "{\"seq\":0,\"t_ms\":0,\"op\":{\"AddMod",
    )
    .unwrap();
    // One that parses but names a module kind kabl doesn't have.
    let mut e = PatchEditor::new();
    e.add_module("no.such.module", Vec2::default());
    kabl_core::save(&sounds.join("alien"), e.log()).unwrap();
    // Unreadable metadata falls back to the directory name.
    let odd = sounds.join("odd");
    kabl_core::save(&odd, init_keyboard().log()).unwrap();
    fs::write(odd.join(library::META_FILE), "name = [").unwrap();

    let lib = open(tmp.path());
    assert!(matches!(
        lib.read("user:broken"),
        Err(LibError::Unreadable(_))
    ));
    let err = lib.read("user:alien").unwrap_err();
    assert!(matches!(err, LibError::Uncompilable(_)), "{err:?}");
    assert!(err.to_string().contains("no.such.module"), "{err}");
    assert_eq!(lib.get("user:odd").unwrap().meta.name, "odd");
    assert!(lib.notes.iter().any(|n| n.contains("odd")));
    assert!(lib.read("user:odd").is_ok());
    assert!(matches!(
        lib.read("user:gone"),
        Err(LibError::Unreadable(_))
    ));
}

#[test]
fn old_patch_directories_without_metadata_are_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("sounds/old-song");
    fs::create_dir_all(&old).unwrap();
    for f in ["log.jsonl", "meta.toml", "checkpoint.json"] {
        fs::copy(factory().join("interlocking").join(f), old.join(f)).unwrap();
    }
    let lib = open(tmp.path());
    let e = lib.get("user:old-song").unwrap();
    assert_eq!(e.meta.name, "old-song");
    assert!(e.meta.sequence && !e.meta.keys);
    assert!(lib.read(&e.id).is_ok());
}

#[test]
fn favorites_and_recents_persist_and_missing_ones_are_kept_until_forgotten() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    assert!(lib.toggle_favorite("factory:palette/lead").is_none());
    lib.touch("factory:palette/pad");
    lib.touch("user:deleted-later");
    lib.touch("factory:palette/pad");
    let mut lib = open(tmp.path());
    assert!(lib.prefs.favorites.contains("factory:palette/lead"));
    assert_eq!(
        lib.prefs.recents,
        ["factory:palette/pad", "user:deleted-later"],
        "newest first, no duplicates"
    );
    assert!(lib.get("user:deleted-later").is_none());
    lib.forget("user:deleted-later");
    lib.toggle_favorite("factory:palette/lead");
    let lib = open(tmp.path());
    assert_eq!(lib.prefs.recents, ["factory:palette/pad"]);
    assert!(lib.prefs.favorites.is_empty());
    for i in 0..30 {
        let mut l = open(tmp.path());
        l.touch(&format!("user:s{i}"));
    }
    assert_eq!(open(tmp.path()).prefs.recents.len(), library::MAX_RECENTS);
}

#[test]
fn a_missing_factory_directory_is_a_note_not_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::open(None, tmp.path().to_path_buf());
    assert!(lib.entries.is_empty());
    assert!(lib.notes[0].contains("KABL_FACTORY_DIR"));
    let lib = Library::open(Some(tmp.path().join("nowhere")), tmp.path().to_path_buf());
    assert!(lib.entries.is_empty());
}

#[test]
fn factory_paths_are_recognized_for_the_advanced_folder_save() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = open(tmp.path());
    assert!(lib.is_factory_path(&factory().join("palette/pad")));
    assert!(lib.is_factory_path(&factory().join("new/sub/dir")));
    assert!(lib.is_factory_path(&factory().join("palette/../reference")));
    assert!(!lib.is_factory_path(&tmp.path().join("x")));
}

#[test]
fn slugs_are_plain() {
    assert_eq!(library::slug("  My  Pad!! 2 "), "my-pad-2");
    assert_eq!(library::slug("???"), "sound");
    assert_eq!(library::slug("Čelo"), "čelo");
}

// ---- D01-R1 repairs (docs/find-play-save/repair-1) ----

fn files(d: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(d)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn a_replacement_must_name_the_sound_it_replaces() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let named = |n: &str| Meta {
        name: n.into(),
        ..Meta::default()
    };
    let a = lib.save(e.log(), named("Alpha"), None).unwrap();
    let before = dir_bytes(&lib.get(&a).unwrap().dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    // A stale confirmation: target Alpha, name Beta.
    let err = lib.save(e2.log(), named("Beta"), Some(&a)).unwrap_err();
    assert!(matches!(err, LibError::ReplaceMismatch { .. }), "{err:?}");
    assert_eq!(dir_bytes(&lib.get(&a).unwrap().dir), before);
    assert!(lib.get("user:beta").is_none());
    // Legitimate: Save in place and a replacement of the sound with that name.
    assert_eq!(lib.save(e2.log(), named("ALPHA"), Some(&a)).unwrap(), a);
    assert_eq!(lib.read(&a).unwrap().state(), e2.state());
}

#[test]
fn a_user_save_failing_at_any_stage_keeps_the_saved_sound() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let dir = lib.get(&id).unwrap().dir.clone();
    let before = dir_bytes(&dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    for stage in ["staged", "replace:dir"] {
        library::fail_saves_at(Some(stage));
        let err = lib.save(e2.log(), meta.clone(), Some(&id)).unwrap_err();
        library::fail_saves_at(None);
        assert!(err.to_string().contains(stage), "{err}");
        assert_eq!(dir_bytes(&dir), before, "{stage}");
        let names: Vec<_> = fs::read_dir(tmp.path().join("sounds"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["keep"], "{stage}: nothing left over");
    }
    assert_eq!(lib.read(&id).unwrap().state(), e.state());
}

#[test]
fn a_folder_save_writes_the_patch_and_keeps_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    // A new folder.
    library::save_folder(&dir, e.log()).unwrap();
    assert_eq!(kabl_core::load(&dir).unwrap().state(), e.state());
    // An existing one with the user's own files and metadata.
    fs::write(dir.join("notes.txt"), "mine").unwrap();
    fs::write(dir.join(library::META_FILE), "name = \"Song\"\n").unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.set_param(3, "cutoff_hz", 900.0);
    library::save_folder(&dir, e2.log()).unwrap();
    assert_eq!(library::read_patch(&dir).unwrap().state(), e2.state());
    assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"mine");
    assert_eq!(
        fs::read(dir.join(library::META_FILE)).unwrap(),
        b"name = \"Song\"\n"
    );
    assert!(!dir.join(".kabl-save").exists());
}

#[test]
fn a_folder_save_failing_at_any_stage_leaves_the_folder_as_it_was() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    kabl_core::save(&dir, e.log()).unwrap();
    fs::write(dir.join("notes.txt"), "mine").unwrap();
    let before = files(&dir);
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.set_param(3, "cutoff_hz", 900.0);
    for stage in [
        "staged",
        "replace:meta.toml",
        "replace:checkpoint.json",
        "replace:log.jsonl",
    ] {
        library::fail_saves_at(Some(stage));
        let err = library::save_folder(&dir, e2.log()).unwrap_err();
        library::fail_saves_at(None);
        assert!(matches!(err, LibError::Io(_)), "{stage}: {err:?}");
        assert_eq!(files(&dir), before, "{stage}");
        assert!(!dir.join(".kabl-save").exists(), "{stage}");
        assert_eq!(kabl_core::load(&dir).unwrap().state(), e.state());
    }
    // A new folder whose first save fails is left without patch files, not half of them.
    let fresh = tmp.path().join("fresh");
    library::fail_saves_at(Some("replace:log.jsonl"));
    assert!(library::save_folder(&fresh, e2.log()).is_err());
    library::fail_saves_at(None);
    assert!(files(&fresh).is_empty());
}

/// Builds the state a folder save killed part way leaves: staging with kabl's marker in
/// `phase`, the new copy staged, and `moved` files already swapped (old aside, new in).
fn crashed_save(dir: &Path, new: &PatchEditor, phase: &str, moved: &[&str]) {
    let staging = dir.join(".kabl-save");
    kabl_core::save(&staging, new.log()).unwrap();
    fs::write(staging.join("kabl-staging"), phase).unwrap();
    for f in moved {
        if dir.join(f).exists() {
            fs::rename(dir.join(f), staging.join(format!("{f}.old"))).unwrap();
        }
        fs::rename(staging.join(f), dir.join(f)).unwrap();
    }
}

#[test]
fn an_interrupted_folder_save_is_repaired_on_the_next_open() {
    let tmp = tempfile::tempdir().unwrap();
    let old = init_keyboard();
    let mut new = PatchEditor::from_log(old.log().clone());
    new.set_param(3, "cutoff_hz", 900.0);
    let complete = |name: &str| {
        let dir = tmp.path().join(name);
        kabl_core::save(&dir, old.log()).unwrap();
        (dir.clone(), files(&dir))
    };

    // Killed with meta and checkpoint swapped and the old log moved aside: rolled back.
    let (dir, before) = complete("a");
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    fs::rename(dir.join("log.jsonl"), dir.join(".kabl-save/log.jsonl.old")).unwrap();
    let note = library::repair_folder(&dir).expect("says it restored");
    assert!(note.contains("undone"), "{note}");
    assert_eq!(files(&dir), before);
    assert!(!dir.join(".kabl-save").exists());

    // Killed after the new log moved in, before cleanup: the save stands.
    let (dir, _) = complete("b");
    crashed_save(
        &dir,
        &new,
        "replacing",
        &["meta.toml", "checkpoint.json", "log.jsonl"],
    );
    assert_eq!(library::read_patch(&dir).unwrap().state(), new.state());
    assert!(!dir.join(".kabl-save").exists());

    // Killed while the new copy was still being written: the folder never changed.
    let (dir, before) = complete("c");
    crashed_save(&dir, &new, "writing", &[]);
    assert_eq!(library::read_patch(&dir).unwrap().state(), old.state());
    assert_eq!(files(&dir), before);
    assert!(!dir.join(".kabl-save").exists());

    // A folder without checkpoint.json: the checkpoint the save added goes on rollback.
    let (dir, _) = complete("d");
    fs::remove_file(dir.join("checkpoint.json")).unwrap();
    let before = files(&dir);
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    library::repair_folder(&dir);
    assert_eq!(files(&dir), before, "no stray checkpoint");

    // A brand-new folder: rollback leaves no half patch.
    let dir = tmp.path().join("e");
    fs::create_dir_all(&dir).unwrap();
    crashed_save(&dir, &new, "replacing", &["meta.toml", "checkpoint.json"]);
    library::repair_folder(&dir);
    assert!(files(&dir).is_empty());
    assert!(!dir.join(".kabl-save").exists());
}

#[test]
fn a_kabl_save_directory_kabl_didnt_make_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let e = init_keyboard();
    kabl_core::save(&dir, e.log()).unwrap();
    fs::create_dir_all(dir.join(".kabl-save")).unwrap();
    fs::write(dir.join(".kabl-save/precious.txt"), "mine").unwrap();
    fs::write(dir.join(".kabl-save/log.jsonl.old"), "not kabl's").unwrap();
    let before = files(&dir);
    assert!(library::repair_folder(&dir).is_none());
    assert!(library::read_patch(&dir).is_ok());
    let err = library::save_folder(&dir, e.log()).unwrap_err();
    assert!(err.to_string().contains("in the way"), "{err}");
    assert_eq!(files(&dir), before);
    assert_eq!(
        fs::read(dir.join(".kabl-save/precious.txt")).unwrap(),
        b"mine"
    );
    // Even kabl's own staging keeps anything else put in it.
    let other = tmp.path().join("other");
    kabl_core::save(&other, e.log()).unwrap();
    crashed_save(&other, &e, "writing", &[]);
    fs::write(other.join(".kabl-save/extra.txt"), "x").unwrap();
    library::repair_folder(&other);
    assert_eq!(fs::read(other.join(".kabl-save/extra.txt")).unwrap(), b"x");
}

#[test]
fn a_retry_after_an_interrupted_user_save_keeps_the_only_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = open(tmp.path());
    let e = edited_pad(&lib);
    let meta = Meta {
        name: "Keep".into(),
        ..Meta::default()
    };
    let id = lib.save(e.log(), meta.clone(), None).unwrap();
    let sounds = tmp.path().join("sounds");
    // What an Interrupted save leaves: the only copy aside as .keep.old.
    fs::rename(sounds.join("keep"), sounds.join(".keep.old")).unwrap();
    let mut e2 = PatchEditor::from_log(e.log().clone());
    e2.undo();
    library::fail_saves_at(Some("replace:dir"));
    assert!(lib.save(e2.log(), meta, Some(&id)).is_err());
    library::fail_saves_at(None);
    assert_eq!(
        lib.read(&id).unwrap().state(),
        e.state(),
        "the old copy survives"
    );
}
