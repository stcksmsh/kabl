//! Composite face icon selectors: which params take built-in glyphs or PNGs, PNG checks, a click
//! per segment, and importing a PNG per option on the design page.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement},
    CompositeId, PortRef,
};
use kabl_ui::{composites, panels, show, PatchEditor, UiState};
use std::io::Cursor;

/// The default patch's oscillator, envelope and VCA.
const OSC: u64 = 2;
const ENV: u64 = 4;
const VCA: u64 = 5;

fn param(id: u64, param: &str) -> PortRef {
    PortRef::Param {
        id,
        param: param.into(),
    }
}

fn png(name: &str, shade: u8) -> Artwork {
    let image = image::RgbaImage::from_pixel(24, 24, image::Rgba([255, 255, 255, shade]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    Artwork {
        name: name.into(),
        png: bytes.into_inner(),
    }
}

fn pngs(n: usize) -> Vec<Artwork> {
    (0..n)
        .map(|k| png(&format!("icon-{k}.png"), 30 * k as u8 + 60))
        .collect()
}

fn icons(icons: Vec<Artwork>, width: f32) -> Placement {
    Placement {
        kind: Kind::Icons,
        x: 12.0,
        y: 72.0,
        width,
        height: 55.0,
        source: None,
        label: None,
        icons,
    }
}

/// The default patch with an LFO, all of it but the MIDI input and output as one composite.
fn patch() -> (PatchEditor, CompositeId, u64) {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let lfo = e.add_module("lfo", kabl_core::Vec2 { x: 40.0, y: 400.0 });
    let id = composites::encapsulate(
        &mut e,
        [OSC, 3, ENV, VCA, lfo].into(),
        Default::default(),
        "Shapes",
    )
    .unwrap();
    (e, id, lfo)
}

/// `panels::validate`'s verdict on the composite's face holding only `target` as `placement`,
/// and the exposure's key.
fn verdict(target: PortRef, placement: Placement) -> (Result<(), String>, u64) {
    let (mut e, id, _) = patch();
    composites::add_exposure(&mut e, id, target, true).unwrap();
    let mut state = e.state().clone();
    let c = state.composites.get_mut(&id).unwrap();
    let key = *c.controls.keys().max().unwrap();
    let mut p = Panel::default();
    p.placements.insert(key, placement);
    c.panel = Some(Box::new(p));
    (panels::validate(&state), key)
}

#[test]
fn built_in_glyphs_are_accepted_only_on_the_waveform_selectors() {
    let lfo = patch().2;
    verdict(param(OSC, "waveform"), icons(Vec::new(), 84.0))
        .0
        .unwrap();
    verdict(param(lfo, "waveform"), icons(Vec::new(), 96.0))
        .0
        .unwrap();
    for target in [param(ENV, "timing"), param(VCA, "exponential")] {
        let (r, key) = verdict(target, icons(Vec::new(), 84.0));
        assert_eq!(
            r.unwrap_err(),
            format!("Binding {key}: no built-in icons for this parameter; import a PNG per option")
        );
    }
}

#[test]
fn pngs_are_accepted_on_any_stepped_param_with_one_per_option() {
    let lfo = patch().2;
    for (target, n) in [
        (param(VCA, "exponential"), 2),
        (param(ENV, "timing"), 2),
        (param(OSC, "waveform"), 4),
        (param(lfo, "waveform"), 5),
    ] {
        verdict(target.clone(), icons(pngs(n), 96.0)).0.unwrap();
        for wrong in [n - 1, n + 1] {
            let (r, key) = verdict(target.clone(), icons(pngs(wrong), 120.0));
            assert_eq!(
                r.unwrap_err(),
                format!("Binding {key}: {wrong} icons for {n} options")
            );
        }
    }
    // A continuous parameter takes no icons at all.
    let (r, key) = verdict(param(3, "cutoff_hz"), icons(pngs(2), 96.0));
    assert_eq!(
        r.unwrap_err(),
        format!("Binding {key}: icon selectors need a stepped parameter")
    );
}

#[test]
fn a_bad_png_is_refused_with_the_artwork_message() {
    let mut set = pngs(2);
    set[1].png = b"GIF89a, not a PNG".to_vec();
    let want = set[1].validate().unwrap_err();
    let (r, _) = verdict(param(VCA, "exponential"), icons(set, 84.0));
    assert_eq!(r.unwrap_err(), want);
}

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
}

impl H {
    /// `module` alone as a composite whose face is one icon selector of `placement` on `name`.
    fn face(module: u64, name: &str, placement: Placement) -> Self {
        let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let id =
            composites::encapsulate(&mut e, [module].into(), Default::default(), "Icons").unwrap();
        composites::add_exposure(&mut e, id, param(module, name), true).unwrap();
        let mut c = e.state().composites[&id].clone();
        let key = *c.controls.keys().next().unwrap();
        let mut panel = Panel::default();
        panel.placements.insert(key, placement);
        c.panel = Some(Box::new(panel));
        composites::set(&mut e, id, c).unwrap();
        Self::with(e)
    }

    fn with(editor: PatchEditor) -> Self {
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            t: 0.0,
            events: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn frames(&mut self, n: usize) {
        let size = egui::vec2(1440.0, 900.0);
        for _ in 0..n {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(self.t),
                events: std::mem::take(&mut self.events),
                ..Default::default()
            };
            self.t += 1.0 / 60.0;
            self.ctx.begin_pass(raw);
            let mut root = egui::Ui::new(
                self.ctx.clone(),
                egui::Id::new("root"),
                egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
            );
            show(&mut self.editor, &mut self.ui, &mut root);
            // Nothing is painted: the tests read the state and hit rects.
            let _ = self.ctx.end_pass();
        }
    }

    fn button(&mut self, p: Pos2, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        self.frames(1);
    }

    fn hit(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit rect {key}"))
    }

    fn click(&mut self, key: &str) {
        let p = self.hit(key).center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        self.button(p, true);
        self.button(p, false);
        self.frames(2);
    }

    fn undo_depth(&self) -> usize {
        self.editor.log().entries().len()
    }

    /// Clicks every segment of `module`'s `name` selector in turn, from the option after
    /// `start` round to `start`: each selects its option as one undo step.
    fn click_every_segment(&mut self, module: u64, name: &str, start: usize, n: usize) {
        let rects: Vec<Rect> = (0..n)
            .map(|k| self.hit(&format!("sel:{module}.{name}.{k}")))
            .collect();
        for (k, r) in rects.iter().enumerate() {
            assert!(r.width() > 0.0, "segment {k} has no area");
            if let Some(next) = rects.get(k + 1) {
                assert!(r.max.x <= next.min.x, "segments {k} and {} overlap", k + 1);
            }
        }
        for k in (start + 1..n).chain(0..=start) {
            let depth = self.undo_depth();
            self.click(&format!("sel:{module}.{name}.{k}"));
            assert_eq!(
                self.editor.state().modules[&module].params[name],
                k as f32,
                "segment {k}"
            );
            assert_eq!(self.undo_depth(), depth + 1, "segment {k}");
        }
    }
}

#[test]
fn every_built_in_glyph_segment_selects_its_option_as_one_undo_step() {
    let mut h = H::face(OSC, "waveform", icons(Vec::new(), 120.0));
    // The default patch's oscillator starts on the saw.
    h.click_every_segment(OSC, "waveform", 2, 4);
    // At the smallest zoom the segments shrink but stay apart and clickable.
    h.ui.zoom = kabl_ui::MIN_ZOOM;
    h.frames(2);
    h.click_every_segment(OSC, "waveform", 2, 4);
    assert_eq!(h.editor.state().modules[&OSC].params["waveform"], 2.0);
}

#[test]
fn every_png_segment_selects_its_option_as_one_undo_step() {
    let mut h = H::face(VCA, "exponential", icons(pngs(2), 84.0));
    h.click_every_segment(VCA, "exponential", 0, 2);
    h.ui.zoom = kabl_ui::MIN_ZOOM;
    h.frames(2);
    h.click_every_segment(VCA, "exponential", 0, 2);
    let depth = h.undo_depth();
    h.editor.undo();
    assert_eq!(h.editor.state().modules[&VCA].params["exponential"], 1.0);
    assert_eq!(h.undo_depth(), depth - 1);
}

#[test]
fn the_design_page_imports_a_png_per_option_and_keeps_each_image_once() {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let id = composites::encapsulate(&mut e, [VCA].into(), Default::default(), "Amp").unwrap();
    composites::add_exposure(&mut e, id, param(VCA, "exponential"), true).unwrap();
    let key = *e.state().composites[&id].controls.keys().next().unwrap();
    let mut h = H::with(e);
    let c = h.editor.state().composites[&id].clone();
    panels::open(&mut h.ui, id, &c);
    h.frames(2);
    h.click(&format!("design:palette:{key}"));
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<String> = ["a.png", "b.png"]
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let path = dir.path().join(name);
            std::fs::write(&path, png(name, 100 + 100 * k as u8).png).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    fn page(h: &H) -> &kabl_ui::design_page::Page {
        h.ui.panels.page.as_ref().expect("the design page is open")
    }
    let placed = |h: &H| page(h).draft.panel.placements[&key].clone();
    let import = |h: &mut H, file: &str, k: usize| {
        h.ui.panels.path = file.to_string();
        h.frames(1);
        h.click(&format!("design:icon:{k}"));
    };
    // The first PNG waits for the second; together they make the selector an icon selector.
    import(&mut h, &files[0], 0);
    assert_eq!(placed(&h).kind, Kind::Selector);
    assert!(placed(&h).icons.is_empty());
    import(&mut h, &files[1], 1);
    let p = placed(&h);
    assert_eq!(p.kind, Kind::Icons, "{:?}", page(&h).note);
    let names: Vec<&str> = p.icons.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["a.png", "b.png"]);
    // Re-importing PNGs the history already holds adds nothing to the pool.
    import(&mut h, &files[0], 1);
    let pool = page(&h).pool_len();
    assert_eq!(pool, 2, "each distinct image once");
    import(&mut h, &files[1], 1);
    import(&mut h, &files[0], 1);
    import(&mut h, &files[0], 0);
    assert_eq!(page(&h).pool_len(), pool);
    h.click("design:undo");
    let names: Vec<String> = placed(&h).icons.iter().map(|a| a.name.clone()).collect();
    assert_eq!(names, ["a.png", "b.png"]);
    assert_eq!(
        placed(&h).icons[1],
        png("b.png", 200),
        "undo restores the bytes"
    );
    // A file that is not a PNG is refused with the artwork message; the draft keeps its icons.
    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"not a png at all").unwrap();
    import(&mut h, &bad.to_string_lossy(), 0);
    let want = Artwork {
        name: "bad.png".into(),
        png: b"not a png at all".to_vec(),
    }
    .validate()
    .unwrap_err();
    assert_eq!(page(&h).note.as_deref(), Some(want.as_str()));
    assert_eq!(placed(&h).icons.len(), 2);
    h.click("design:apply");
    let applied = h.editor.state().composites[&id].panel.clone().unwrap();
    assert_eq!(applied.placements[&key].kind, Kind::Icons);
    assert_eq!(applied.placements[&key].icons.len(), 2);
}

#[test]
fn a_partial_icon_set_is_dropped_by_remove_kind_change_and_undo() {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let id = composites::encapsulate(&mut e, [VCA].into(), Default::default(), "Amp").unwrap();
    composites::add_exposure(&mut e, id, param(VCA, "exponential"), true).unwrap();
    let key = *e.state().composites[&id].controls.keys().next().unwrap();
    let mut h = H::with(e);
    let c = h.editor.state().composites[&id].clone();
    panels::open(&mut h.ui, id, &c);
    h.frames(2);
    h.click(&format!("design:palette:{key}"));
    // Stages a.png for option 0, then `drop` must forget it.
    let stage_then = |h: &mut H, drop: &dyn Fn(&mut H)| {
        let state = h.editor.state().clone();
        let page = h.ui.panels.page.as_mut().unwrap();
        page.set_icon(&state, key, 0, png("a.png", 100)).unwrap();
        assert_eq!(page.icons(&state, key)[0], Some(png("a.png", 100)));
        drop(h);
        h.frames(1);
        let page = h.ui.panels.page.as_ref().unwrap();
        assert_eq!(page.icons(&state, key), vec![None, None]);
    };
    stage_then(&mut h, &|h| {
        h.ui.panels.page.as_mut().unwrap().remove(key);
        h.frames(1);
        h.click(&format!("design:palette:{key}"));
    });
    stage_then(&mut h, &|h| {
        let state = h.editor.state().clone();
        let page = h.ui.panels.page.as_mut().unwrap();
        page.set_kind(&state, key, Kind::Toggle).unwrap();
        page.set_kind(&state, key, Kind::Selector).unwrap();
    });
    stage_then(&mut h, &|h| {
        assert!(h.ui.panels.page.as_mut().unwrap().undo());
    });
}
