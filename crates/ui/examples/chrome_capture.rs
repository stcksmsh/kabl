//! Scene captures for the G5 chrome PRs: the real app (`kabl_ui::show`) with the factory sound
//! Acid Line, in the default Signal solid (dark) and Drafting solid (light) looks, at the three
//! window sizes of PLAN §8, for the Rack, Perform and Sounds scenes.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT_DIR [scene,scene|all] [theme]`
//!
//! Files: `OUT/<scene>__<dark|light>__<W>x<H>.png`. `theme` is a theme id (default
//! `kabl.line_solid`; `kabl.satin` for a before/after). Silent: no audio device is opened.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

const SIZES: [(u32, u32); 3] = [(1440, 900), (1280, 800), (1180, 680)];
const SCENES: [&str; 25] = [
    "rack",
    "perform",
    "sounds",
    "drawer",
    "wavetable-editor",
    "seq-pattern-editor",
    "empty",
    "error",
    "cues-editor",
    "cues-face",
    "cable-pattern-editor",
    "compare",
    "explain",
    "inspect",
    "learn",
    "cable-routes-drawer",
    "cable-routes-rest",
    "cable-routes-opening",
    "file-open",
    "file-save",
    "file-empty",
    "menu-module",
    "menu-cable",
    "more-controls",
    "more-controls-face",
];

struct Job {
    scene: &'static str,
    dark: bool,
    size: (u32, u32),
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    theme: String,
    editor: PatchEditor,
    ui: UiState,
    _user: tempfile::TempDir,
    /// A control to inspect a few frames before the screenshot (the lanes mid-opening).
    inspect_later: Option<(kabl_core::ModuleId, String)>,
    /// `KABL_ANIM=1`: the lanes animation as frames on a fixed 60 fps clock (see `anim_step`).
    anim: bool,
    /// Frames since start, never reset: the fixed clock.
    tick: u64,
    /// Animation frames saved for this job.
    saved: u32,
    /// Input events for the next frame (a right-click that opens a context menu).
    inject: Vec<egui::Event>,
}

/// Lanes animation timeline, in 60 fps frames after the scene settles: rest 0.5 s, inspect,
/// open (180 ms) and hold 0.7 s, deselect, close and hold 0.5 s.
const ANIM_START: u32 = 12;
const ANIM_OPEN_AT: u32 = 30;
const ANIM_CLOSE_AT: u32 = 30 + 11 + 42;
const ANIM_FRAMES: u32 = ANIM_CLOSE_AT + 11 + 30;

fn patches() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

/// The mockup's patch: sequencer, VA (or, with `wt`, wavetable) oscillator, SVF, ADSR, mixer,
/// delay, reverb, output.
fn mockup_patch(wt: bool) -> PatchEditor {
    use kabl_core::PortRef;
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module(if wt { "osc.wt" } else { "osc.va" }, at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let mix = e.add_module("mixer", at(244.0, 320.0));
    let delay = e.add_module("delay", at(504.0, 320.0));
    let verb = e.add_module("reverb", at(744.0, 320.0));
    let out = e.add_module("out", at(984.0, 320.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(osc, "out"), m(svf, "in"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.connect(m(svf, "lp"), m(mix, "in1"));
    e.connect(m(mix, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    assert_eq!(e.state().cables.len(), 8, "mockup patch cables");
    e
}

/// The Perform mockup as a patch: a clock, four macros (Bite, Wash, Drive, Space on CC 20 to 23,
/// each moving two controls), three sequencers (Bass, Drums, Lead) and eight scenes, so the
/// capture compares like for like. `KABL_CAPTURE_FEW=1` captures the small cue-macros sound
/// instead.
fn perform_mockup_patch() -> PatchEditor {
    use kabl_core::PortRef;
    use kabl_ui::perform::{rename_pin, toggle_pin, BANKS, CUE_PADS, TRANSPORT};
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let clock = e.add_module("clock", at(24.0, 20.0));
    e.set_param(clock, "bpm", 124.0);
    let seqs = [
        e.add_module("seq", at(244.0, 20.0)),
        e.add_module("seq", at(484.0, 20.0)),
        e.add_module("seq", at(724.0, 20.0)),
    ];
    let osc = e.add_module("osc.va", at(24.0, 320.0));
    let svf = e.add_module("filter.svf", at(244.0, 320.0));
    let adsr = e.add_module("env.adsr", at(464.0, 320.0));
    let mix = e.add_module("mixer", at(684.0, 320.0));
    let delay = e.add_module("delay", at(904.0, 320.0));
    let verb = e.add_module("reverb", at(1124.0, 320.0));
    let mac = e.add_module("macro", at(964.0, 20.0));
    let cues = e.add_module("cues", at(1184.0, 20.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    let macros = [
        ("Bite", 0.62, [(svf, "cutoff_hz"), (adsr, "decay_ms")]),
        ("Wash", 0.35, [(verb, "decay_s"), (delay, "mix")]),
        ("Drive", 0.48, [(osc, "unison"), (mix, "level1")]),
        ("Space", 0.70, [(delay, "feedback"), (verb, "mix")]),
    ];
    for (k, (name, v, dst)) in macros.iter().enumerate() {
        let out = format!("m{}", k + 1);
        e.set_label(mac, &format!("name.{out}"), Some((*name).into()));
        e.set_param(mac, &out, *v);
        e.set_param(mac, &format!("cc.{out}"), (20 + k) as f32);
        for &(to, param) in dst {
            e.connect_route(m(mac, &out), to, param);
        }
        toggle_pin(&mut e, mac, &out);
    }
    // Step levels (percent) of the first eight steps of each lane in the mockup.
    let lanes: [(&str, [f32; 8]); 3] = [
        ("Bass", [90.0, 50.0, 0.0, 70.0, 100.0, 30.0, 0.0, 80.0]),
        ("Drums", [100.0, 0.0, 60.0, 0.0, 100.0, 0.0, 60.0, 30.0]),
        ("Lead", [0.0, 70.0, 0.0, 70.0, 0.0, 90.0, 0.0, 50.0]),
    ];
    for (&s, (name, levels)) in seqs.iter().zip(lanes) {
        // Every bank plays the lane (the playing bank is the one the lane shows).
        for prefix in ["", "b.", "c.", "d."] {
            for (k, &v) in levels.iter().enumerate() {
                e.set_param(s, &format!("{prefix}v{}", k + 1), v);
                e.set_param(
                    s,
                    &format!("{prefix}g{}", k + 1),
                    if v > 0.0 { 1.0 } else { 0.0 },
                );
            }
        }
        toggle_pin(&mut e, s, BANKS);
        rename_pin(&mut e, s, BANKS, name);
    }
    toggle_pin(&mut e, clock, TRANSPORT);
    // Scene -> banks of (Bass, Drums, Lead). Scene 3 plays, scene 4 is queued.
    let scenes = [
        ("Intro", [0, 0, 0]),
        ("Build", [0, 0, 1]),
        ("Drop", [1, 0, 2]),
        ("Break", [2, 1, 0]),
        ("Verse", [0, 1, 1]),
        ("Lift", [1, 1, 2]),
        ("Outro", [2, 2, 0]),
        ("Fill", [3, 3, 3]),
    ];
    for (i, (name, banks)) in scenes.iter().enumerate() {
        let n = i + 1;
        e.set_label(cues, &format!("cue{n}"), Some((*name).into()));
        e.set_param(cues, &format!("cue{n}.on"), 1.0);
        e.set_param(cues, &format!("cue{n}.clock"), clock as f32);
        for (&s, &b) in seqs.iter().zip(banks) {
            e.set_param(cues, &format!("cue{n}.seq.{s}"), b as f32);
        }
    }
    toggle_pin(&mut e, cues, CUE_PADS);
    e
}

impl App {
    fn prepare(&mut self) {
        let j = &self.jobs[self.i];
        // KABL_CAPTURE_PATCH=DIR captures another patch than the mockup's.
        self.editor = match std::env::var("KABL_CAPTURE_PATCH") {
            Ok(d) => PatchEditor::from_log(
                kabl_core::load(std::path::Path::new(&d)).expect("capture patch"),
            ),
            // Perform needs pins, cues and macros: the cue-macros sound, with its sequencers'
            // banks pinned too.
            Err(_) if j.scene == "perform" && std::env::var("KABL_CAPTURE_FEW").is_err() => {
                perform_mockup_patch()
            }
            Err(_) if j.scene == "perform" => {
                let dir = patches().join("cue-macros");
                let mut e = PatchEditor::from_log(kabl_core::load(&dir).expect("cue-macros"));
                let seqs: Vec<_> = e
                    .state()
                    .modules
                    .iter()
                    .filter(|(_, m)| m.kind == "seq")
                    .map(|(&id, _)| id)
                    .collect();
                for id in seqs {
                    if !kabl_ui::perform::is_pinned(e.state(), id, kabl_ui::perform::BANKS) {
                        kabl_ui::perform::toggle_pin(&mut e, id, kabl_ui::perform::BANKS);
                    }
                    // The sound leaves every step at its default (100 % level and chance):
                    // vary them so the lane shows what it can.
                    for (k, (v, r)) in [
                        (100.0, 100.0),
                        (55.0, 70.0),
                        (0.0, 100.0),
                        (80.0, 100.0),
                        (100.0, 60.0),
                        (35.0, 100.0),
                        (90.0, 80.0),
                        (60.0, 100.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        e.set_param(id, &format!("v{}", k + 1), v);
                        e.set_param(id, &format!("r{}", k + 1), r);
                    }
                    // Scenes 1 to 4 launch banks A to D of this sequencer.
                    let cues: Vec<_> = e
                        .state()
                        .modules
                        .iter()
                        .filter(|(_, m)| m.kind == "cues")
                        .map(|(&c, _)| c)
                        .collect();
                    for c in cues {
                        for k in 1..=4u32 {
                            e.set_param(c, &format!("cue{k}.seq.{id}"), (k - 1) as f32);
                        }
                    }
                }
                e
            }
            Err(_) => mockup_patch(j.scene == "wavetable-editor"),
        };
        let mut ui = UiState::default();
        ui.theme = self.theme.clone();
        ui.dark = j.dark;
        ui.status_meter = true;
        // The mockup's status bar readouts, as a host would feed them.
        ui.status_cpu = Some(6.0);
        ui.status_buffer = Some(128);
        ui.status_voices = Some((4, 8));
        ui.status_bpm = Some(124.0);
        ui.status_pos = Some("17 . 3 . 2".into());
        ui.library = Some(Library::open(
            Some(patches()),
            self._user.path().to_path_buf(),
        ));
        // The file browser's Sounds place needs the user's sounds folder.
        std::fs::create_dir_all(self._user.path().join("sounds")).unwrap();
        if j.scene == "sounds" {
            ui.browser.selected = Some("factory:presets/acid-line".into());
        }
        let mut doc = kabl_ui::browser::Doc::new(
            "Acid Line",
            kabl_ui::browser::DocOrigin::Library("factory:acid-line".into()),
            self.editor.state(),
        );
        doc.meta.category = "Bass".into();
        ui.doc = Some(doc);
        ui.browser_open = j.scene == "sounds";
        ui.drawer_open = matches!(
            j.scene,
            "rack"
                | "seq-pattern-editor"
                | "cues-editor"
                | "cable-pattern-editor"
                | "cable-routes-drawer"
                | "cable-routes-rest"
                | "cable-routes-opening"
        );
        if j.scene == "empty" {
            self.editor = PatchEditor::new();
        }
        if j.scene.starts_with("file-") {
            use kabl_ui::filebrowser::{self as fb, Mode, Purpose};
            let patches = std::env::temp_dir().join(PATCHES);
            ui.folders.insert("patches".into(), patches.clone());
            let (purpose, title, mode, name) = match j.scene {
                "file-save" => (Purpose::PatchSave, "Save patch", Mode::Save, "my-patch"),
                _ => (Purpose::PatchLoad, "Open patch", Mode::Open, ""),
            };
            if j.scene == "file-empty" {
                ui.folders
                    .insert("patches".into(), patches.with_file_name("empty-set"));
            }
            fb::open(&mut ui, purpose, title, mode, fb::PATCHES, name, "");
            if j.scene != "file-empty" {
                fb::preselect(&mut ui, "acid-line");
            }
        }
        if j.scene == "error" {
            use kabl_ui::kit::g4::{Sev, Toast};
            // Oldest first: the stack shows the newest on top.
            for (sev, title, detail) in [
                (
                    Sev::Bad,
                    "Could not load patch",
                    Some("unknown module kind \"osc.xyz\""),
                ),
                (
                    Sev::Warn,
                    "Audio device busy",
                    Some("Playing silent until it is free."),
                ),
                (Sev::Good, "Saved \"Warm pad\"", None),
            ]
            .into_iter()
            .rev()
            {
                ui.toasts.push(Toast {
                    sev,
                    title: title.into(),
                    detail: detail.map(Into::into),
                    actions: if sev == Sev::Bad {
                        vec!["Show details".into(), "Open another…".into()]
                    } else {
                        vec![]
                    },
                    expanded: true,
                    born: 0.0,
                });
            }
        }
        if matches!(j.scene, "explain" | "inspect" | "learn") {
            ui.drawer_open = true;
        }
        if j.scene == "explain" {
            let state = self.editor.state();
            if let Some((&id, _)) = state.modules.iter().find(|(_, m)| m.kind == "filter.svf") {
                kabl_ui::explain::open(
                    &mut ui,
                    kabl_ui::explain::Subject::Control {
                        id,
                        key: "cutoff_hz".into(),
                    },
                    None,
                );
            }
        }
        if j.scene == "inspect" {
            ui.selected_module = None;
            ui.inspect.open = true;
        }
        if j.scene == "learn" {
            ui.recipes.open = true;
        }
        if j.scene == "compare" {
            ui.drawer_open = true;
            ui.compare.open = true;
            ui.compare.capture(&self.editor, "you");
        }
        ui.perform_open = j.scene == "perform";
        ui.perform_tall = j.scene == "perform";
        if j.scene == "perform" {
            // A moving show: bar 17 beat 3 on the clock, the first scene playing, the second
            // queued for the next bar, and a playhead on every sequencer.
            let state = self.editor.state();
            let ids = |kind: &str| -> Vec<kabl_core::ModuleId> {
                state
                    .modules
                    .iter()
                    .filter(|(_, m)| m.kind == kind)
                    .map(|(&id, _)| id)
                    .collect()
            };
            for c in ids("clock") {
                ui.clock_pos.insert(c, 16.0 * 16.0 + 2.0 * 4.0 + 1.0);
            }
            for s in ids("seq") {
                ui.seq_steps.insert(s, 5);
            }
            for cues in ids("cues") {
                let all = kabl_ui::cues::cues(state, cues);
                if let Some(first) = all.get(2).or(all.first()) {
                    for &(s, bank) in &first.targets {
                        ui.seq_banks.insert(s, (bank, None));
                    }
                    // The next scene that plays a different bank is the queued one.
                    let next = all
                        .iter()
                        .skip(all.len().min(3))
                        .chain(all.iter())
                        .find(|c| {
                            c.targets
                                .iter()
                                .any(|&(s, b)| ui.seq_banks.get(&s).is_some_and(|e| e.0 != b))
                        });
                    for &(s, bank) in next.iter().flat_map(|c| c.targets.iter()) {
                        if let Some(e) = ui.seq_banks.get_mut(&s) {
                            if e.0 != bank {
                                e.1 = Some(bank);
                            }
                        }
                    }
                }
                ui.last_cue.insert(cues, 1);
            }
        }
        // Like the mockup: the oscillator-to-filter cable and the filter are selected.
        ui.selected_route = self
            .editor
            .state()
            .cables
            .iter()
            .find(|(_, c)| matches!(&c.to, kabl_core::PortRef::Module { port, .. } if port == "in"))
            .map(|(&id, _)| id);
        ui.selected_module = self
            .editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "filter.svf")
            .map(|(&id, _)| id);
        if j.scene == "inspect" {
            ui.selected_module = None;
        }
        if j.scene == "seq-pattern-editor" {
            // The mockup's state: editing A, B playing, step 5 under the playhead.
            let seq = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "seq")
                .map(|(&id, _)| id)
                .unwrap();
            let gate = [1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0];
            let vel = [100.0, 80.0, 80.0, 60.0, 100.0, 50.0, 70.0, 90.0];
            let prob = [100.0, 100.0, 100.0, 75.0, 100.0, 50.0, 100.0, 100.0];
            let ratch = [1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 3.0];
            for i in 0..8 {
                self.editor.set_param(seq, &format!("g{}", i + 1), gate[i]);
                self.editor.set_param(seq, &format!("v{}", i + 1), vel[i]);
                self.editor.set_param(seq, &format!("r{}", i + 1), prob[i]);
                self.editor.set_param(seq, &format!("k{}", i + 1), ratch[i]);
            }
            ui.selected_module = Some(seq);
            ui.selected_route = None;
            ui.edit_bank.insert(seq, 0);
            ui.seq_banks.insert(seq, (1, None));
            ui.seq_steps.insert(seq, 4);
        }
        if j.scene == "wavetable-editor" {
            let wt = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "osc.wt")
                .map(|(&id, _)| id)
                .unwrap();
            kabl_ui::tables::open(&self.editor, &mut ui, wt);
            // Like the mockup: frame 7 of 16 selected, the oscillator's position at 0.40.
            ui.tables.select(6);
            self.editor.set_param(wt, "position", 0.4);
        }
        if j.scene == "cable-pattern-editor" {
            // The mockup's state: Seq Pitch to VA Osc Pitch with a 16-step pattern, chance
            // 100 %, morph 35 % moved by the Bite macro, glide 40 ms, and no clock.
            let cable = self
                .editor
                .state()
                .cables
                .iter()
                .find(|(_, c)| {
                    matches!(&c.to, kabl_core::PortRef::Module { port, .. } if port == "pitch")
                })
                .map(|(&id, _)| id)
                .unwrap();
            let on = [1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1];
            let chance = [
                1.0, 1.0, 1.0, 0.75, 1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 0.75, 1.0, 1.0, 0.5, 1.0,
            ];
            let mut set = |slot: usize, v: f32| {
                self.editor.set_param_gesture(
                    kabl_core::ParamTarget::Cable {
                        id: cable,
                        param: kabl_cables::slot_name(slot),
                    },
                    v,
                    true,
                );
            };
            set(0, 16.0);
            set(1, 100.0);
            for k in 0..16 {
                set(kabl_cables::LEVEL0 + k, on[k] as f32);
                set(kabl_cables::CHANCE0 + k, chance[k] * 100.0);
            }
            set(kabl_cables::MORPH, 0.35);
            set(kabl_cables::GLIDE, 40.0);
            let mac = self
                .editor
                .add_module("macro", kabl_core::Vec2 { x: 24.0, y: 640.0 });
            self.editor.set_label(mac, "name.m1", Some("Bite".into()));
            self.editor.connect_cable_route(
                kabl_core::PortRef::Module {
                    id: mac,
                    port: "m1".into(),
                },
                cable,
                "morph",
            );
            ui.cable_fn = Some(cable);
            ui.selected_route = Some(cable);
            ui.selected_module = None;
        }
        if matches!(j.scene, "cues-editor" | "cues-face") {
            // The mockup's state: Bass, Drums and Lead sequencers (two more in a third row,
            // below the view), four cues, Drop playing and Break queued.
            let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
            let bass = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "seq")
                .map(|(&id, _)| id)
                .unwrap();
            let drums = self.editor.add_module("seq", at(24.0, 640.0));
            let lead = self.editor.add_module("seq", at(724.0, 640.0));
            let cues = self.editor.add_module("cues", at(1424.0, 640.0));
            for (s, name) in [(bass, "Bass"), (drums, "Drums"), (lead, "Lead")] {
                kabl_ui::perform::rename_pin(&mut self.editor, s, kabl_ui::perform::BANKS, name);
            }
            // Banks of (Bass, Drums, Lead) per cue; None leaves it ("off" in the mockup).
            let scenes: [(&str, [Option<usize>; 3]); 4] = [
                ("Intro", [Some(0), Some(0), None]),
                ("Build", [Some(0), Some(1), Some(0)]),
                ("Drop", [Some(1), Some(0), Some(2)]),
                ("Break", [None, Some(3), Some(1)]),
            ];
            for (k, (name, banks)) in scenes.into_iter().enumerate() {
                kabl_ui::cues::add(&mut self.editor, cues, &|_| 0);
                kabl_ui::cues::rename(&mut self.editor, cues, k + 1, name);
                for (s, b) in [bass, drums, lead].into_iter().zip(banks) {
                    kabl_ui::cues::set_target(&mut self.editor, cues, k + 1, s, b);
                }
            }
            ui.selected_module = (j.scene == "cues-editor").then_some(cues);
            ui.selected_route = None;
            // Drop plays; Break is queued (Drums waits for D, Lead for B; Bass keeps B).
            ui.seq_banks.insert(bass, (1, None));
            ui.seq_banks.insert(drums, (0, Some(3)));
            ui.seq_banks.insert(lead, (2, Some(1)));
            ui.last_cue.insert(cues, 3);
        }
        if j.scene.starts_with("cable-routes") {
            // The mockup's state: the filter's cutoff at 1 kHz, moved by the envelope (+30 %,
            // selected) and the Bite macro (+38 %, its module below the view).
            let find = |kind: &str| {
                self.editor
                    .state()
                    .modules
                    .iter()
                    .find(|(_, m)| m.kind == kind)
                    .map(|(&id, _)| id)
                    .unwrap()
            };
            let (svf, adsr) = (find("filter.svf"), find("env.adsr"));
            let mac = self
                .editor
                .add_module("macro", kabl_core::Vec2 { x: 24.0, y: 640.0 });
            self.editor.set_label(mac, "name.m1", Some("Bite".into()));
            self.editor.set_param(svf, "cutoff_hz", 1000.0);
            let m = |id, port: &str| kabl_core::PortRef::Module {
                id,
                port: port.into(),
            };
            let env = kabl_ui::routing::routes_into(self.editor.state(), svf, "cutoff_hz")
                .into_iter()
                .find(|r| r.from_id == adsr)
                .unwrap()
                .cable;
            self.editor.set_route_amount(env, 0.30, true);
            let bite = self.editor.connect_route(m(mac, "m1"), svf, "cutoff_hz");
            self.editor.set_route_amount(bite, 0.38, true);
            ui.inspected = Some((svf, "cutoff_hz".into()));
            ui.selected_route = Some(env);
            // At rest the lanes are collapsed; "opening" inspects 3 frames before the shot.
            if j.scene != "cable-routes-drawer" {
                self.inspect_later = ui.inspected.take();
            }
        }
        self.ui = ui;
        self.f = 0;
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        // A headless window has no focus: text fields would show no caret.
        raw.focused = true;
        // Modals centre on the screen rect: make it the capture size, not the real window.
        if let Some(j) = self.jobs.get(self.i) {
            raw.screen_rect = Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(j.size.0 as f32, j.size.1 as f32),
            ));
        }
        // The animation runs on a fixed 60 fps clock, whatever the real frame rate.
        if self.anim {
            raw.time = Some(self.tick as f64 / 60.0);
            raw.predicted_dt = 1.0 / 60.0;
            self.tick += 1;
        }
        raw.events.append(&mut self.inject);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.i >= self.jobs.len() {
            return;
        }
        let mut shot = None;
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                shot = Some(image);
            }
        }
        if let (Some(img), true) = (shot.as_ref(), self.anim) {
            // A 2x crop of the filter's face, one PNG per frame.
            let svf = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "filter.svf")
                .map(|(&id, _)| id)
                .unwrap();
            let r = self.ui.hits[&format!("module:{svf}")].expand(8.0);
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let (x, y) = ((r.left() * 2.0) as u32, (r.top().max(0.0) * 2.0) as u32);
            let cw = ((r.width() * 2.0) as u32).min(w as u32 - x) & !1;
            let ch = ((r.height() * 2.0) as u32).min(h as u32 - y) & !1;
            let crop = image::imageops::crop_imm(&full, x, y, cw, ch).to_image();
            let j = &self.jobs[self.i];
            let name = format!(
                "anim__{}__{:03}.png",
                if j.dark { "dark" } else { "light" },
                self.saved
            );
            crop.save(self.dir.join(&name)).unwrap();
            self.saved += 1;
            if self.saved == ANIM_FRAMES {
                println!("wrote {} frames", self.saved);
                self.saved = 0;
                self.i += 1;
                if self.i >= self.jobs.len() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                self.prepare();
            }
        } else if let Some(img) = shot {
            let j = &self.jobs[self.i];
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let crop = image::imageops::crop_imm(
                &full,
                0,
                0,
                j.size.0.min(w as u32),
                j.size.1.min(h as u32),
            )
            .to_image();
            let name = format!(
                "{}__{}__{}x{}.png",
                j.scene,
                if j.dark { "dark" } else { "light" },
                j.size.0,
                j.size.1
            );
            crop.save(self.dir.join(&name)).unwrap();
            println!("wrote {name}");
            self.i += 1;
            if self.i >= self.jobs.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            // Modals keep their position in memory: a smaller size must re-centre them.
            ctx.memory_mut(|m| m.reset_areas());
            self.prepare();
        }
        let size = self.jobs[self.i].size;
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(size.0 as f32, size.1 as f32),
            )),
        );
        if self.f == 6
            && matches!(
                self.jobs[self.i].scene,
                "drawer"
                    | "seq-pattern-editor"
                    | "cues-editor"
                    | "cable-pattern-editor"
                    | "cable-routes-drawer"
                    | "cable-routes-rest"
                    | "cable-routes-opening"
            )
        {
            // After the first frame: a compact window closes the drawer there, the user opens it.
            self.ui.drawer_open = true;
        }
        show(&mut self.editor, &mut self.ui, &mut root);
        // The mockup's rack zoom (95 %, 78 % at 1180); the extra modules sit below the view.
        if self.f == 6
            && matches!(
                self.jobs[self.i].scene,
                "cues-editor"
                    | "cable-pattern-editor"
                    | "cable-routes-drawer"
                    | "cable-routes-rest"
                    | "cable-routes-opening"
            )
        {
            self.ui.zoom = if self.jobs[self.i].size.0 < 1280 {
                0.78
            } else {
                0.95
            };
        }
        // The cues face at full size: the third row, where the extra sequencers and the cues sit.
        if self.f == 6 && self.jobs[self.i].scene == "cues-face" {
            self.ui.zoom = 1.0;
            self.ui.pan = egui::vec2(-700.0, -560.0);
        }
        // The mockup's More controls: the wavetable oscillator's, Controls tab (once the first
        // frames, which reset the view, are past).
        if self.f == 6 && self.jobs[self.i].scene.starts_with("more-controls") {
            let wt = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "osc.va")
                .map(|(&id, _)| id)
                .unwrap();
            self.ui.expanded.insert(wt);
            self.ui.more_tab = usize::from(self.jobs[self.i].scene == "more-controls-face");
        }
        // The menus: a right-click on the sequencer's title bar, or on a cable's middle. The
        // previous job's menu is still open in egui's memory (the press would only close it):
        // Escape it first.
        if self.f == 6 && matches!(self.jobs[self.i].scene, "menu-module" | "menu-cable") {
            self.inject.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if matches!(self.f, 8 | 9)
            && matches!(self.jobs[self.i].scene, "menu-module" | "menu-cable")
        {
            let state = self.editor.state();
            let find = |kind: &str| {
                state
                    .modules
                    .iter()
                    .find(|(_, m)| m.kind == kind)
                    .map(|(&id, _)| id)
                    .unwrap()
            };
            let at = if self.jobs[self.i].scene == "menu-module" {
                let r = self.ui.hits[&format!("module:{}", find("seq"))];
                r.left_top() + egui::vec2(r.width() * 0.25, 22.0)
            } else {
                let (osc, svf) = (find("osc.va"), find("filter.svf"));
                let c = state
                    .cables
                    .iter()
                    .find(|(_, c)| {
                        matches!((&c.from, &c.to), (
                            kabl_core::PortRef::Module { id: a, .. },
                            kabl_core::PortRef::Module { id: b, .. },
                        ) if *a == osc && *b == svf)
                    })
                    .map(|(&id, _)| id)
                    .unwrap();
                self.ui.hits[&format!("cable:{c}")].center()
            };
            self.inject.push(egui::Event::PointerMoved(at));
            self.inject.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed: self.f == 8,
                modifiers: egui::Modifiers::NONE,
            });
        }
        // The lanes' animation runs on the clock: give the routes scenes time to settle.
        let shot = if self.jobs[self.i].scene.starts_with("cable-routes")
            || self.jobs[self.i].scene.starts_with("menu-")
            || self.jobs[self.i].scene.starts_with("more-controls")
        {
            30
        } else {
            12
        };
        if self.f == shot - 3 && self.jobs[self.i].scene == "cable-routes-opening" {
            self.ui.inspected = self.inspect_later.take();
        }
        if self.anim {
            // The face alone: no drawer over the filter.
            self.ui.drawer_open = false;
            let t = self.f.wrapping_sub(ANIM_START);
            if t == ANIM_OPEN_AT {
                self.ui.inspected = self.inspect_later.take();
            }
            if t == ANIM_CLOSE_AT {
                self.inspect_later = self.ui.inspected.take();
            }
            if self.f >= ANIM_START && t < ANIM_FRAMES {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            self.f += 1;
            ctx.request_repaint();
            return;
        }
        self.f += 1;
        if self.f == shot {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

/// The file browser scenes' folders, as in the mockup: `HOME/Music/kabl/patches` with patch
/// folders of fixed dates, and an empty `empty-set` beside it. HOME is pointed at the tree's
/// root so the breadcrumb starts at Home. Called before any thread starts (it sets HOME).
fn file_tree() {
    let home = std::env::temp_dir().join("kabl-capture-home");
    let patches = std::env::temp_dir().join(PATCHES);
    std::fs::create_dir_all(patches.with_file_name("empty-set")).unwrap();
    // Real folders, then patches (a folder with meta.toml) with the mockup's dates (UTC).
    for name in ["composition-batch", "cue-macros", "effects"] {
        std::fs::create_dir_all(patches.join(name)).unwrap();
    }
    for (name, at) in [
        ("acid-line", 1_793_644_800u64),
        ("dense-composition", 1_793_571_300),
        ("rope-walk", 1_793_476_920),
        ("slab-city", 1_793_360_820),
        ("warm-pad", 1_793_179_800),
    ] {
        let d = patches.join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.toml"), "schema_version = 8\n").unwrap();
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(at);
        let _ = std::fs::File::open(&d).and_then(|f| f.set_modified(t));
    }
    std::env::set_var("HOME", &home);
}

/// The patches folder of [`file_tree`], under the temporary folder.
const PATCHES: &str = "kabl-capture-home/Music/kabl/patches";

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.get(1).is_none_or(|w| w == "all" || w.contains("file-")) {
        file_tree();
    }
    let dir = PathBuf::from(a.first().cloned().unwrap_or_else(|| "target/chrome".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let want = a.get(1).cloned().unwrap_or_else(|| "all".into());
    let theme = a
        .get(2)
        .cloned()
        .unwrap_or_else(|| "kabl.line_solid".into());
    let anim = std::env::var("KABL_ANIM").is_ok();
    let mut jobs = Vec::new();
    if anim {
        for dark in [true, false] {
            jobs.push(Job {
                scene: "cable-routes-rest",
                dark,
                size: (1440, 900),
            });
        }
    }
    for scene in SCENES.iter().copied().filter(|_| !anim) {
        if want != "all" && !want.split(',').any(|w| w == scene) {
            continue;
        }
        for dark in [true, false] {
            for size in SIZES {
                jobs.push(Job { scene, dark, size });
            }
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(if anim {
                [2880.0, 1800.0]
            } else {
                [1440.0, 900.0]
            })
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "chrome capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            if anim {
                cc.egui_ctx.set_zoom_factor(2.0);
            }
            let mut app = App {
                jobs,
                i: 0,
                f: 0,
                dir,
                theme,
                editor: PatchEditor::seed_from(&Default::default()),
                ui: UiState::default(),
                _user: tempfile::tempdir().unwrap(),
                inspect_later: None,
                anim,
                tick: 0,
                saved: 0,
                inject: Vec::new(),
            };
            app.prepare();
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
