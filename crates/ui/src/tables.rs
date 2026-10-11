//! The wavetable editor: a window opened from an `osc.wt` face. It holds a working copy of the
//! table's frames, edits them (pen, harmonic bars, frame and spectral operations, import of a
//! .wav) and writes the result into the patch as the embedded table of the module's slot
//! (`PatchEditor::put_table`), so the oscillator plays every edit through the ordinary
//! compile-and-crossfade swap and undo, redo, save and the plugin state all work unchanged.
//! Control thread only.

use crate::kit::{self, Button, Tone};
use crate::style::{alpha, Role, Style};
use crate::{PatchEditor, UiState};
use egui::{pos2, vec2, Pos2, Rect, Sense, Shape, Stroke};
use kabl_core::{table::MAX_TABLES, ModuleId};
use kabl_engine::patch_engine::{Command, MAX_PREVIEW_NOTES, MAX_PREVIEW_SECS};
use kabl_modules::tableedit as ed;
use kabl_modules::wavetable::{
    self, Detected, ImportInfo, FACTORY_COUNT, FACTORY_NAMES, FRAME, FRAME_CHOICES, MAX_FRAMES,
    MAX_FRAME_SAMPLES,
};

/// Seconds between table writes while a stroke is down: each write is one compile and
/// crossfade, and the table's mipmaps are rebuilt for it.
const COMMIT_EVERY: f64 = 0.12;
/// Samples faded at each end by "Fade edges".
const FADE: usize = 128;

struct Pending {
    bytes: Vec<u8>,
    name: String,
    /// `None`: detect the frame length.
    choice: Option<usize>,
    result: Result<(Vec<Vec<f32>>, ImportInfo), String>,
}

#[derive(Default)]
pub struct View {
    editor: u64,
    pub(crate) module: Option<ModuleId>,
    slot: u64,
    name: String,
    frames: Vec<Vec<f32>>,
    /// The table bytes in the patch when this view last wrote or read them; a different value
    /// there means undo, redo or a load changed the table under the view.
    synced: Vec<u8>,
    sel: usize,
    mark: Option<usize>,
    bars: bool,
    steps: usize,
    path: String,
    error: Option<String>,
    pending: Option<Pending>,
    /// A stroke is down and its undo step is open.
    stroke_open: bool,
    /// The working copy has edits not yet written to the patch.
    dirty: bool,
    last_commit: f64,
    last_pt: Option<(usize, f32)>,
    audition: bool,
}

/// Opens the editor on `module`'s table: its embedded table, or an editable copy of its factory
/// table in the first free slot (written, and the module pointed at it, on the first edit).
pub fn open(editor: &PatchEditor, ui: &mut UiState, module: ModuleId) {
    let state = editor.state();
    let Some(m) = state.modules.get(&module) else {
        return;
    };
    let user = m.params.get("user").copied().unwrap_or(0.0).round().max(0.0) as u64;
    let slot = if (1..=MAX_TABLES).contains(&user) {
        user
    } else if let Some(s) = (1..=MAX_TABLES).find(|s| !state.tables.contains_key(s)) {
        s
    } else {
        ui.last_message = Some(format!(
            "All {MAX_TABLES} table slots hold a table; point this oscillator at one to edit it"
        ));
        return;
    };
    ui.tables = View {
        editor: editor.instance(),
        module: Some(module),
        slot,
        steps: 4,
        ..View::default()
    };
    ui.tables.load(editor);
}

impl View {
    /// The last import or write error shown in the window.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The selected frame.
    pub fn selected(&self) -> usize {
        self.sel
    }

    /// Frames in the working copy.
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    /// Frames the pending import would make, if one is waiting for "Use as table".
    pub fn pending_frames(&self) -> Option<usize> {
        match &self.pending {
            Some(Pending { result: Ok((f, _)), .. }) => Some(f.len()),
            _ => None,
        }
    }

    fn factory_index(&self, editor: &PatchEditor) -> usize {
        self.module
            .and_then(|id| editor.state().modules.get(&id))
            .and_then(|m| m.params.get("table"))
            .map_or(0, |&t| t.round().clamp(0.0, (FACTORY_COUNT - 1) as f32) as usize)
    }

    /// (Re)reads the working copy from the patch, or from the factory table when the slot is empty.
    fn load(&mut self, editor: &PatchEditor) {
        match editor.state().tables.get(&self.slot) {
            Some(t) => {
                self.frames = wavetable::decode_canonical(&t.wav).unwrap_or_default();
                self.name = t.name.clone();
                self.synced = t.wav.clone();
            }
            None => {
                let i = self.factory_index(editor);
                self.frames = wavetable::factory_frames(i);
                self.name = wavetable::table_name(&format!("{}.wav", FACTORY_NAMES[i]));
                self.synced.clear();
            }
        }
        if self.frames.is_empty() {
            self.frames = vec![ed::sine()];
        }
        self.sel = self.sel.min(self.frames.len() - 1);
        self.mark = self.mark.filter(|&m| m < self.frames.len() && m != self.sel);
        self.dirty = false;
    }

    /// Writes the working copy into the patch. `first` starts an undo step.
    fn commit(&mut self, editor: &mut PatchEditor, first: bool) {
        ed::fit(&mut self.frames);
        let wav = wavetable::canonical_wav(&self.frames);
        let bound = self.module.is_some_and(|id| {
            editor
                .state()
                .modules
                .get(&id)
                .and_then(|m| m.params.get("user"))
                .is_some_and(|&u| u.round() as u64 == self.slot)
        });
        let bind = self.module.filter(|_| !bound);
        match editor.put_table(self.slot, &self.name, wav.clone(), first, bind) {
            Ok(()) => {
                self.synced = wav;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        self.dirty = false;
    }

    /// One undo step for a whole operation.
    fn apply(&mut self, editor: &mut PatchEditor, f: impl FnOnce(&mut Self)) {
        f(self);
        self.sel = self.sel.min(self.frames.len() - 1);
        self.mark = self.mark.filter(|&m| m < self.frames.len() && m != self.sel);
        self.commit(editor, true);
        self.stroke_open = false;
    }

    /// Pen bookkeeping each frame: write on a throttle while `down`, finish on release.
    fn pen(&mut self, editor: &mut PatchEditor, down: bool, now: f64) {
        if down {
            if self.dirty && (!self.stroke_open || now - self.last_commit >= COMMIT_EVERY) {
                self.commit(editor, !self.stroke_open);
                self.stroke_open = true;
                self.last_commit = now;
            }
        } else if self.stroke_open {
            if self.dirty {
                self.commit(editor, false);
            }
            self.stroke_open = false;
            self.last_pt = None;
        }
    }

    /// The selected frames: `sel` alone, or `sel` to `mark` inclusive.
    fn range(&self) -> std::ops::RangeInclusive<usize> {
        match self.mark {
            Some(m) => self.sel.min(m)..=self.sel.max(m),
            None => self.sel..=self.sel,
        }
    }

    fn read_file(&mut self) {
        use std::io::Read;
        let path = std::path::Path::new(self.path.trim());
        let read = || -> Result<Vec<u8>, String> {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let meta = file.metadata().map_err(|e| e.to_string())?;
            if !meta.is_file() {
                return Err("not a file".into());
            }
            let mut bytes = Vec::new();
            file.take(wavetable::MAX_IMPORT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            Ok(bytes)
        };
        match read() {
            Ok(bytes) => {
                let file = path
                    .file_name()
                    .map_or(String::new(), |f| f.to_string_lossy().into_owned());
                let mut p = Pending {
                    bytes,
                    name: wavetable::table_name(&file),
                    choice: None,
                    result: Err(String::new()),
                };
                p.refresh();
                self.error = None;
                self.pending = Some(p);
            }
            Err(e) => {
                self.pending = None;
                self.error = Some(format!("{}: {e}", path.display()));
            }
        }
    }
}

impl Pending {
    fn refresh(&mut self) {
        self.result = wavetable::import_wav_with(&self.bytes, self.choice)
            .map_err(|e| e.to_string())
            .and_then(|(wav, info)| {
                wavetable::decode_canonical(&wav)
                    .map(|f| (f, info))
                    .map_err(|e| e.to_string())
            });
    }

    fn text(info: &ImportInfo) -> String {
        let how = match info.detected {
            Detected::Clm => "named by the file's clm chunk",
            Detected::Multiple => "a multiple of 2048 samples, no clm chunk",
            Detected::Cycle => "no clm chunk and not a multiple of 2048: one cycle",
            Detected::Chosen => "chosen",
        };
        let tail = if info.dropped > 0 {
            format!("; the last {} samples are dropped", info.dropped)
        } else {
            String::new()
        };
        format!(
            "{} frame{} of {} samples ({how}){tail}",
            info.frames,
            if info.frames == 1 { "" } else { "s" },
            info.frame_len
        )
    }
}

/// `n` points along one cycle of `f`, closed back to its first sample.
fn cycle_pts(rect: Rect, f: &[f32], n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let s = f[(i * FRAME / n) % FRAME].clamp(-1.0, 1.0);
            pos2(
                rect.left() + rect.width() * i as f32 / n as f32,
                rect.center().y - s * rect.height() * 0.5,
            )
        })
        .collect()
}

fn stack(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    v: &mut View,
    frames: &[Vec<f32>],
    size: egui::Vec2,
    preview: bool,
) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    ui_state.record("wt:stack".into(), rect);
    let p = ui.painter_at(rect);
    kit::face::display_frame(&p, st, rect, 1.0);
    let n = frames.len();
    let inner = rect.shrink(8.0);
    let amp = inner.height() * 0.22;
    let step = (inner.height() - amp * 2.0).max(1.0) / n.max(2) as f32;
    let skew = inner.width() * 0.22 / n.max(2) as f32;
    let wave_w = inner.width() * 0.76;
    let base = |k: usize| pos2(inner.left() + skew * k as f32, inner.bottom() - amp - step * k as f32);
    let lane = |k: usize| {
        let b = base(k);
        Rect::from_min_size(pos2(b.x, b.y - amp), vec2(wave_w, amp * 2.0))
    };
    let trace = kit::face::trace_color(st);
    for k in (0..n).rev() {
        let (sel, mark) = (!preview && k == v.sel, !preview && v.mark == Some(k));
        let pts = cycle_pts(lane(k), &frames[k], 96);
        let (col, w) = if sel {
            (st.roles.accent, 2.2)
        } else if mark {
            (st.roles.warn, 1.8)
        } else {
            (alpha(trace, 150), 1.0)
        };
        p.add(Shape::line(pts, Stroke::new(w, col)));
    }
    kit::face::playhead(&p, st, base(v.sel.min(n - 1)) + vec2(-4.0, 0.0), 0.6);
    if let (false, Some(at)) = (preview, resp.interact_pointer_pos()) {
        if resp.clicked() {
            let k = (0..n)
                .min_by(|&a, &b| {
                    let (da, db) = ((at.y - base(a).y).abs(), (at.y - base(b).y).abs());
                    da.total_cmp(&db)
                })
                .unwrap_or(0);
            if ui.input(|i| i.modifiers.shift) && k != v.sel {
                v.mark = Some(k);
            } else {
                v.sel = k;
                v.mark = None;
            }
        }
    }
}

fn wave(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    editor: &mut PatchEditor,
    v: &mut View,
    size: egui::Vec2,
    now: f64,
) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    ui_state.record("wt:wave".into(), rect);
    let p = ui.painter_at(rect);
    kit::face::display_frame(&p, st, rect, 1.0);
    let down = resp.is_pointer_button_down_on();
    if let (true, Some(at)) = (down, ui.input(|i| i.pointer.latest_pos())) {
        let x = ((at.x - rect.left()) / rect.width()).clamp(0.0, 0.9999);
        let idx = (x * FRAME as f32) as usize;
        let val = (1.0 - 2.0 * (at.y - rect.top()) / rect.height()).clamp(-1.0, 1.0);
        let (i0, v0) = v.last_pt.unwrap_or((idx, val));
        let (lo, hi) = (i0.min(idx), i0.max(idx));
        let f = &mut v.frames[v.sel];
        for i in lo..=hi {
            let t = if hi == lo {
                1.0
            } else if i0 <= idx {
                (i - lo) as f32 / (hi - lo) as f32
            } else {
                (hi - i) as f32 / (hi - lo) as f32
            };
            f[i] = v0 + (val - v0) * t;
        }
        v.last_pt = Some((idx, val));
        v.dirty = true;
    }
    v.pen(editor, down, now);
    let f = &v.frames[v.sel];
    kit::face::trace(&p, st, cycle_pts(rect.shrink2(vec2(0.0, 6.0)), f, 512), 1.0, None);
}

fn bars(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    editor: &mut PatchEditor,
    v: &mut View,
    size: egui::Vec2,
    now: f64,
) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    ui_state.record("wt:bars".into(), rect);
    let p = ui.painter_at(rect);
    kit::face::display_frame(&p, st, rect, 1.0);
    let w = rect.width() / ed::BARS as f32;
    let down = resp.is_pointer_button_down_on();
    if let (true, Some(at)) = (down, ui.input(|i| i.pointer.latest_pos())) {
        let h = (((at.x - rect.left()) / w) as usize).min(ed::BARS - 1) + 1;
        // Bars are drawn on a square-root scale, so the quiet upper harmonics are reachable.
        let y = (1.0 - (at.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        ed::set_harmonic(&mut v.frames[v.sel], h, y * y);
        v.dirty = true;
    }
    v.pen(editor, down, now);
    let amps = ed::harmonics(&v.frames[v.sel]);
    for (i, a) in amps.iter().enumerate() {
        let hgt = a.sqrt().min(1.0) * rect.height();
        let bar = Rect::from_min_max(
            pos2(rect.left() + w * i as f32 + 1.0, rect.bottom() - hgt),
            pos2(rect.left() + w * (i + 1) as f32 - 1.0, rect.bottom()),
        );
        p.rect_filled(bar, 1.0, kit::face::trace_color(st));
    }
}

fn button(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    key: &str,
    text: &str,
    enabled: bool,
) -> bool {
    let r = ui.add(Button::new(st, text).small().enabled(enabled));
    ui_state.record(format!("wt:{key}"), r.rect);
    r.clicked()
}

fn body(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    editor: &mut PatchEditor,
    v: &mut View,
    now: f64,
) {
    let n = v.frames.len();
    ui.horizontal(|ui| {
        kit::label(
            ui,
            st,
            Role::H3,
            Tone::Text,
            format!("{} · {n} frame{} · slot {}", v.name, if n == 1 { "" } else { "s" }, v.slot),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, st, ui_state, "redo", "Redo", true) {
                editor.redo();
            }
            if button(ui, st, ui_state, "undo", "Undo", true) {
                editor.undo();
            }
            let label = if v.audition { "Stop" } else { "Audition" };
            if button(ui, st, ui_state, "audition", label, true) {
                if v.audition {
                    ui_state.launches.push(Command::PreviewStop);
                } else {
                    let mut notes = [0u8; MAX_PREVIEW_NOTES];
                    notes[0] = 48;
                    ui_state.launches.push(Command::Preview {
                        notes,
                        count: 1,
                        velocity: 100,
                        blocks: (MAX_PREVIEW_SECS * ui_state.sample_rate
                            / kabl_engine::graph::BLOCK as f32) as u32,
                    });
                }
                v.audition = !v.audition;
            }
        });
    });
    ui.horizontal(|ui| {
        let r = kit::field(ui, st, &mut v.path, "Path to a .wav file", None, Some(420.0));
        ui_state.record("wt:path".into(), r.rect);
        if button(ui, st, ui_state, "import", "Import", !v.path.trim().is_empty()) {
            v.read_file();
        }
    });
    if let Some(e) = &v.error {
        kit::label(ui, st, Role::Caption, Tone::Bad, e);
    }
    let mut use_pending = false;
    let mut cancel = false;
    let mut preview: Option<Vec<Vec<f32>>> = None;
    if let Some(pd) = &mut v.pending {
        match &pd.result {
            Ok((frames, info)) => {
                kit::label(ui, st, Role::Caption, Tone::Text2, Pending::text(info));
                if info.ambiguous() {
                    kit::label(
                        ui,
                        st,
                        Role::Caption,
                        Tone::Warn,
                        "Could be one long cycle instead: pick the frame size below.",
                    );
                }
                preview = Some(frames.clone());
            }
            Err(e) => {
                kit::label(ui, st, Role::Caption, Tone::Bad, e);
            }
        }
        let whole = wavetable::import_wav_with(&pd.bytes, None)
            .ok()
            .map(|(_, i)| i.samples)
            .filter(|&s| s <= MAX_FRAME_SAMPLES);
        let mut choice = pd.choice;
        ui.horizontal(|ui| {
            kit::label(ui, st, Role::Caption, Tone::Text3, "Frame size");
            kit::segmented(ui, st, |ui| {
                let r = kit::seg(ui, st, "Auto", choice.is_none());
                ui_state.record("wt:size-auto".into(), r.rect);
                if r.clicked() {
                    choice = None;
                }
                for &c in &FRAME_CHOICES {
                    let r = kit::seg(ui, st, &c.to_string(), choice == Some(c));
                    ui_state.record(format!("wt:size-{c}"), r.rect);
                    if r.clicked() {
                        choice = Some(c);
                    }
                }
                if let Some(s) = whole {
                    let r = kit::seg(ui, st, "Whole file", choice == Some(s));
                    ui_state.record("wt:size-whole".into(), r.rect);
                    if r.clicked() {
                        choice = Some(s);
                    }
                }
            });
        });
        if choice != pd.choice {
            pd.choice = choice;
            pd.refresh();
        }
        ui.horizontal(|ui| {
            let ok = pd.result.is_ok();
            let r = ui.add(Button::new(st, "Use as table").primary().small().enabled(ok));
            ui_state.record("wt:use".into(), r.rect);
            use_pending = r.clicked();
            cancel = button(ui, st, ui_state, "cancel", "Cancel", true);
        });
    }
    if use_pending {
        if let Some(Pending {
            name,
            result: Ok((frames, _)),
            ..
        }) = v.pending.take()
        {
            v.apply(editor, |v| {
                v.frames = frames;
                v.name = name;
                v.sel = 0;
                v.mark = None;
            });
        }
    } else if cancel {
        v.pending = None;
    }

    ui.add_space(st.sp(1));
    let shown = preview.as_deref().unwrap_or(&v.frames).to_vec();
    ui.horizontal_top(|ui| {
        stack(ui, st, ui_state, v, &shown, vec2(300.0, 300.0), preview.is_some());
        ui.vertical(|ui| {
            kit::segmented(ui, st, |ui| {
                let r = kit::seg(ui, st, "Draw", !v.bars);
                ui_state.record("wt:tab-draw".into(), r.rect);
                if r.clicked() {
                    v.bars = false;
                }
                let r = kit::seg(ui, st, "Harmonics", v.bars);
                ui_state.record("wt:tab-bars".into(), r.rect);
                if r.clicked() {
                    v.bars = true;
                }
            });
            let size = vec2(440.0, 268.0);
            if preview.is_some() {
                kit::label(ui, st, Role::Body, Tone::Text2, "Previewing the import.");
            } else if v.bars {
                bars(ui, st, ui_state, editor, v, size, now);
            } else {
                wave(ui, st, ui_state, editor, v, size, now);
            }
        });
    });

    ui.add_space(st.sp(1));
    let (sel, room, can_morph) = (
        v.sel,
        n < MAX_FRAMES,
        v.mark.is_some() && v.mark != Some(v.sel),
    );
    let editing = preview.is_none();
    ui.horizontal_wrapped(|ui| {
        if button(ui, st, ui_state, "insert", "Insert", editing && room) {
            v.apply(editor, |v| v.frames.insert(sel + 1, ed::sine()));
            v.sel += 1;
        }
        if button(ui, st, ui_state, "dup", "Duplicate", editing && room) {
            v.apply(editor, |v| {
                let f = v.frames[sel].clone();
                v.frames.insert(sel + 1, f);
            });
            v.sel += 1;
        }
        if button(ui, st, ui_state, "delete", "Delete", editing && n > 1) {
            v.apply(editor, |v| {
                v.frames.remove(sel);
            });
        }
        if button(ui, st, ui_state, "left", "Move earlier", editing && sel > 0) {
            v.apply(editor, |v| v.frames.swap(sel, sel - 1));
            v.sel -= 1;
        }
        if button(ui, st, ui_state, "right", "Move later", editing && sel + 1 < n) {
            v.apply(editor, |v| v.frames.swap(sel, sel + 1));
            v.sel += 1;
        }
        ui.separator();
        let r = v.range();
        if button(ui, st, ui_state, "normalise", "Normalise", editing) {
            v.apply(editor, |v| r.for_each(|k| ed::normalise(&mut v.frames[k])));
        }
        if button(ui, st, ui_state, "dc", "Remove DC", editing) {
            let r = v.range();
            v.apply(editor, |v| r.for_each(|k| ed::remove_dc(&mut v.frames[k])));
        }
        if button(ui, st, ui_state, "fade", "Fade edges", editing) {
            let r = v.range();
            v.apply(editor, |v| r.for_each(|k| ed::fade_edges(&mut v.frames[k], FADE)));
        }
    });
    ui.horizontal_wrapped(|ui| {
        kit::label(ui, st, Role::Caption, Tone::Text3, "Shift-click a second frame, then fill between:");
        ui.add(egui::DragValue::new(&mut v.steps).range(1..=16).suffix(" new"));
        for (key, text, spectral) in [("interp", "Interpolate", false), ("morph", "Morph", true)] {
            if button(ui, st, ui_state, key, text, editing && can_morph) {
                let (a, b) = (v.sel.min(v.mark.unwrap_or(0)), v.sel.max(v.mark.unwrap_or(0)));
                let steps = v.steps;
                v.apply(editor, |v| {
                    // Adjacent frames: make room for `steps` new ones between them.
                    let b = if b == a + 1 {
                        let add = steps.min(MAX_FRAMES - v.frames.len());
                        for _ in 0..add {
                            v.frames.insert(a + 1, ed::sine());
                        }
                        b + add
                    } else {
                        b
                    };
                    ed::fill_between(&mut v.frames, a, b, spectral);
                });
            }
        }
    });
}

/// The editor window, if one is open. Call once per frame.
pub fn show(editor: &mut PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    let Some(id) = ui_state.tables.module else {
        return;
    };
    if ui_state.tables.editor != editor.instance() || !editor.state().modules.contains_key(&id) {
        ui_state.tables = View::default();
        return;
    }
    let mut v = std::mem::take(&mut ui_state.tables);
    let in_patch = editor
        .state()
        .tables
        .get(&v.slot)
        .map_or(&[][..], |t| t.wav.as_slice());
    if in_patch != v.synced.as_slice() && !v.stroke_open {
        v.load(editor);
    }
    let st = ui_state.style.clone();
    let now = ctx.input(|i| i.time);
    let mut open = true;
    egui::Window::new("Wavetable editor")
        .id(egui::Id::new("wt-editor"))
        .open(&mut open)
        .frame(kit::popover_frame(&st))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| body(ui, &st, ui_state, editor, &mut v, now));
    if v.audition && !open {
        ui_state.launches.push(Command::PreviewStop);
    }
    if open {
        ui_state.tables = v;
    }
}
