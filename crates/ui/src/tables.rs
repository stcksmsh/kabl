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
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

/// Seconds between table writes while a stroke is down: each write is one compile and
/// crossfade, and the table's mipmaps are rebuilt for it.
const COMMIT_EVERY: f64 = 0.12;
/// Samples faded at each end by "Fade edges".
const FADE: usize = 128;

/// Frames of a table, shared.
type Frames = Arc<Vec<Vec<f32>>>;

/// Rows the file browser shows for one folder (a folder with more is cut, and says so).
const MAX_ENTRIES: usize = 2000;
/// Directory entries one listing looks at before it gives up (a folder of millions of files).
const MAX_SCANNED: usize = 100_000;

/// One row of the file browser: a folder, or a .wav file.
struct Entry {
    name: String,
    dir: bool,
}

/// A folder's rows (and whether the list was cut), or why it could not be read.
type Listing = Result<(Vec<Entry>, bool), String>;

/// The file browser: one folder, listed on a thread of its own so a slow or huge folder (a
/// network share) never holds the interface or, in a plugin, the host's window.
struct Browse {
    dir: PathBuf,
    rx: Option<mpsc::Receiver<Listing>>,
    listing: Option<Listing>,
}

impl Browse {
    fn new(dir: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let there = dir.clone();
        std::thread::spawn(move || {
            let _ = tx.send(list(&there));
        });
        Browse {
            dir,
            rx: Some(rx),
            listing: None,
        }
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(l) => self.listing = Some(l),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.listing = Some(Err("the folder could not be read".into()))
            }
        }
        self.rx = None;
    }
}

/// Folders first, then .wav files, each by name without regard to case; hidden names left out.
fn list(dir: &Path) -> Listing {
    let mut rows = Vec::new();
    let mut cut = false;
    for (seen, entry) in std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .enumerate()
    {
        if seen >= MAX_SCANNED {
            cut = true;
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        let wav = Path::new(&name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        if meta.is_dir() || (meta.is_file() && wav) {
            if rows.len() >= MAX_ENTRIES {
                cut = true;
                break;
            }
            rows.push(Entry {
                name,
                dir: meta.is_dir(),
            });
        }
    }
    rows.sort_by_key(|e| (!e.dir, e.name.to_lowercase()));
    Ok((rows, cut))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

struct Pending {
    bytes: Vec<u8>,
    name: String,
    /// `None`: detect the frame length.
    choice: Option<usize>,
    /// The whole file as one cycle, when it is short enough to offer ("Whole file").
    whole: Option<usize>,
    /// The frames the import would make (shared, so drawing them each frame copies nothing).
    result: Result<(Frames, ImportInfo), String>,
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
    /// The folder of the last file picked or browsed, kept while the application runs.
    last_dir: Option<PathBuf>,
    browse: Option<Browse>,
    /// A stroke is down and its undo step is open.
    stroke_open: bool,
    /// The working copy has edits not yet written to the patch.
    dirty: bool,
    last_commit: f64,
    last_pt: Option<(usize, f32)>,
    audition: bool,
    /// A drag of the position slider is in progress (its undo step is open).
    pos_open: bool,
}

/// Opens the editor on `module`'s table: its embedded table, or an editable copy of its factory
/// table in the first free slot (written, and the module pointed at it, on the first edit).
pub fn open(editor: &PatchEditor, ui: &mut UiState, module: ModuleId) {
    let state = editor.state();
    let Some(m) = state.modules.get(&module) else {
        return;
    };
    let user = m
        .params
        .get("user")
        .copied()
        .unwrap_or(0.0)
        .round()
        .max(0.0) as u64;
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
        last_dir: ui.tables.last_dir.take(),
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
            Some(Pending {
                result: Ok((f, _)), ..
            }) => Some(f.len()),
            _ => None,
        }
    }

    fn factory_index(&self, editor: &PatchEditor) -> usize {
        self.module
            .and_then(|id| editor.state().modules.get(&id))
            .and_then(|m| m.params.get("table"))
            .map_or(0, |&t| {
                t.round().clamp(0.0, (FACTORY_COUNT - 1) as f32) as usize
            })
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
        self.mark = self
            .mark
            .filter(|&m| m < self.frames.len() && m != self.sel);
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
        self.mark = self
            .mark
            .filter(|&m| m < self.frames.len() && m != self.sel);
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
        let path = PathBuf::from(self.path.trim());
        let read = || -> Result<Vec<u8>, String> {
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
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
                if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                    self.last_dir = Some(dir.to_path_buf());
                }
                let file = path
                    .file_name()
                    .map_or(String::new(), |f| f.to_string_lossy().into_owned());
                self.start_pending(bytes, &file);
            }
            Err(e) => {
                self.pending = None;
                self.error = Some(format!("{}: {e}", path.display()));
            }
        }
    }

    /// Previews `bytes` (a file's contents) as an import named after `file`.
    fn start_pending(&mut self, bytes: Vec<u8>, file: &str) {
        let mut p = Pending {
            bytes,
            name: wavetable::table_name(file),
            choice: None,
            whole: None,
            result: Err(String::new()),
        };
        p.refresh();
        self.error = None;
        self.pending = Some(p);
    }

    /// Opens the browser in the folder last used, else the path field's folder, else home.
    fn start_browse(&mut self) {
        let from_path = Path::new(self.path.trim())
            .parent()
            .filter(|d| d.is_dir())
            .map(Path::to_path_buf);
        let dir = self
            .last_dir
            .clone()
            .filter(|d| d.is_dir())
            .or(from_path)
            .or_else(home)
            .unwrap_or_else(|| PathBuf::from("/"));
        self.browse = Some(Browse::new(dir));
    }
}

impl Pending {
    fn refresh(&mut self) {
        self.result = wavetable::import_wav_with(&self.bytes, self.choice)
            .map_err(|e| e.to_string())
            .and_then(|(wav, info)| {
                wavetable::decode_canonical(&wav)
                    .map(|f| (Arc::new(f), info))
                    .map_err(|e| e.to_string())
            });
        if let (None, Ok((_, info))) = (self.whole, &self.result) {
            self.whole = (info.samples <= MAX_FRAME_SAMPLES).then_some(info.samples);
        }
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
    let base = |k: usize| {
        pos2(
            inner.left() + skew * k as f32,
            inner.bottom() - amp - step * k as f32,
        )
    };
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
        for (k, s) in f[lo..=hi].iter_mut().enumerate() {
            let t = if hi == lo {
                1.0
            } else if i0 <= idx {
                k as f32 / (hi - lo) as f32
            } else {
                (hi - lo - k) as f32 / (hi - lo) as f32
            };
            *s = v0 + (val - v0) * t;
        }
        v.last_pt = Some((idx, val));
        v.dirty = true;
    }
    v.pen(editor, down, now);
    let f = &v.frames[v.sel];
    kit::face::trace(
        &p,
        st,
        cycle_pts(rect.shrink2(vec2(0.0, 6.0)), f, 512),
        1.0,
        None,
    );
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

/// The stack of frames beside the pen or bars (the import preview while one is pending).
fn main_area(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    editor: &mut PatchEditor,
    v: &mut View,
    preview: &Option<Frames>,
    now: f64,
) {
    ui.horizontal_top(|ui| {
        let size = vec2(300.0, 280.0);
        match &preview {
            Some(frames) => stack(ui, st, ui_state, v, frames, size, true),
            None => {
                // The stack reads only the selection from `v`: lend it the frames by value.
                let frames = std::mem::take(&mut v.frames);
                stack(ui, st, ui_state, v, &frames, size, false);
                v.frames = frames;
            }
        }
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
            let size = vec2(440.0, 248.0);
            if preview.is_some() {
                kit::label(ui, st, Role::Body, Tone::Text2, "Previewing the import.");
            } else if v.bars {
                bars(ui, st, ui_state, editor, v, size, now);
            } else {
                wave(ui, st, ui_state, editor, v, size, now);
            }
        });
    });
}

/// The folder list that replaces the main area while browsing for a file.
fn browser(ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState, v: &mut View) {
    let Some(b) = &mut v.browse else { return };
    b.poll();
    if b.rx.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(30));
    }
    let (mut go, mut pick, mut close) = (None::<PathBuf>, None::<String>, false);
    ui.horizontal(|ui| {
        if button(ui, st, ui_state, "up", "Up", b.dir.parent().is_some()) {
            go = b.dir.parent().map(Path::to_path_buf);
        }
        if button(ui, st, ui_state, "home", "Home", home().is_some()) {
            go = home();
        }
        if button(ui, st, ui_state, "browse-close", "Close", true) {
            close = true;
        }
        kit::label_truncated(
            ui,
            st,
            Role::Caption,
            Tone::Text2,
            b.dir.display().to_string(),
        );
    });
    let height = 280.0;
    egui::ScrollArea::vertical()
        .id_salt("wt-browse")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| match &b.listing {
            None => {
                kit::label(ui, st, Role::Body, Tone::Text2, "Reading the folder...");
            }
            Some(Err(e)) => {
                kit::label(ui, st, Role::Body, Tone::Bad, e);
            }
            Some(Ok((rows, cut))) => {
                if rows.is_empty() {
                    kit::label(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text2,
                        "No folders or .wav files here.",
                    );
                }
                for e in rows {
                    let text = if e.dir {
                        format!("{}/", e.name)
                    } else {
                        e.name.clone()
                    };
                    let r = kit::menu_item(ui, st, &text, false);
                    let key = if e.dir { "dir" } else { "file" };
                    ui_state.record(format!("wt:{key}:{}", e.name), r.rect);
                    if r.clicked() {
                        if e.dir {
                            go = Some(b.dir.join(&e.name));
                        } else {
                            pick = Some(b.dir.join(&e.name).to_string_lossy().into_owned());
                        }
                    }
                }
                if *cut {
                    kit::label(
                        ui,
                        st,
                        Role::Caption,
                        Tone::Warn,
                        "The list is cut: type the path instead.",
                    );
                }
            }
        });
    if let Some(dir) = go {
        v.last_dir = Some(dir.clone());
        v.browse = Some(Browse::new(dir));
    } else if let Some(path) = pick {
        v.path = path;
        v.browse = None;
        v.read_file();
    } else if close {
        v.browse = None;
    }
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
            format!(
                "{} · {n} frame{} · slot {}",
                v.name,
                if n == 1 { "" } else { "s" },
                v.slot
            ),
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
                            / kabl_engine::graph::BLOCK as f32)
                            as u32,
                    });
                }
                v.audition = !v.audition;
            }
        });
    });
    ui.horizontal(|ui| {
        let r = kit::field(
            ui,
            st,
            &mut v.path,
            "Path to a .wav file",
            None,
            Some(420.0),
        );
        ui_state.record("wt:path".into(), r.rect);
        ui.add_space(st.sp(1));
        if button(ui, st, ui_state, "browse", "Browse", true) {
            if v.browse.is_some() {
                v.browse = None;
            } else {
                v.start_browse();
            }
        }
        if button(
            ui,
            st,
            ui_state,
            "import",
            "Import",
            !v.path.trim().is_empty(),
        ) {
            v.read_file();
        }
    });
    if let Some(e) = &v.error {
        kit::label(ui, st, Role::Caption, Tone::Bad, e);
    }
    let mut use_pending = false;
    let mut cancel = false;
    let mut preview: Option<Arc<Vec<Vec<f32>>>> = None;
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
                preview = Some(Arc::clone(frames));
            }
            Err(e) => {
                kit::label(ui, st, Role::Caption, Tone::Bad, e);
            }
        }
        let whole = pd.whole;
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
            let r = ui.add(
                Button::new(st, "Use as table")
                    .primary()
                    .small()
                    .enabled(ok),
            );
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
                v.frames = Arc::try_unwrap(frames).unwrap_or_else(|shared| (*shared).clone());
                v.name = name;
                v.sel = 0;
                v.mark = None;
            });
        }
    } else if cancel {
        v.pending = None;
    }

    ui.add_space(st.sp(1));
    if v.browse.is_some() {
        browser(ui, st, ui_state, v);
    } else {
        main_area(ui, st, ui_state, editor, v, &preview, now);
    }

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
        if button(
            ui,
            st,
            ui_state,
            "right",
            "Move later",
            editing && sel + 1 < n,
        ) {
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
            v.apply(editor, |v| {
                r.for_each(|k| ed::fade_edges(&mut v.frames[k], FADE))
            });
        }
    });
    // The module's own position, to sweep the frames while auditioning.
    if let Some(id) = v.module {
        let target = kabl_core::ParamTarget::Module {
            id,
            param: "position".into(),
        };
        let mut pos = f64::from(editor.state().param(&target).unwrap_or(0.0));
        ui.horizontal(|ui| {
            kit::label(ui, st, Role::Caption, Tone::Text3, "Position");
            let readout = format!("{pos:.2}");
            let r = kit::slider(ui, st, &mut pos, 0.0..=1.0, None, &readout);
            ui_state.record("wt:position".into(), r.rect);
            if r.changed() {
                editor.set_param_gesture(target, pos as f32, !v.pos_open);
                v.pos_open = true;
            }
            if !r.dragged() {
                v.pos_open = false;
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        kit::label(
            ui,
            st,
            Role::Caption,
            Tone::Text3,
            "Shift-click a second frame, then fill between:",
        );
        ui.add(
            egui::DragValue::new(&mut v.steps)
                .range(1..=16)
                .suffix(" new"),
        );
        for (key, text, spectral) in [("interp", "Interpolate", false), ("morph", "Morph", true)] {
            if button(ui, st, ui_state, key, text, editing && can_morph) {
                let (a, b) = (
                    v.sel.min(v.mark.unwrap_or(0)),
                    v.sel.max(v.mark.unwrap_or(0)),
                );
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
/// A .wav dropped on the editor window, or on an `osc.wt` display (which opens the editor for
/// that module), is previewed as an import. Other files are ignored. Needs a host that passes
/// file drops to the application: the standalone window does.
fn dropped(editor: &PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    let files = ctx.input(|i| i.raw.dropped_files.clone());
    let wav = |name: &str| {
        Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
    };
    let Some(file) = files.into_iter().find(|f| {
        f.path
            .as_ref()
            .map_or_else(|| wav(&f.name), |p| wav(&p.to_string_lossy()))
    }) else {
        return;
    };
    if ui_state.tables.module.is_none() {
        let at = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()));
        let target = at.and_then(|at| {
            ui_state.hits.iter().find_map(|(k, r)| {
                let id = k.strip_prefix("wt-display:")?.parse::<ModuleId>().ok()?;
                r.contains(at).then_some(id)
            })
        });
        let Some(id) = target else { return };
        open(editor, ui_state, id);
    }
    let v = &mut ui_state.tables;
    match (file.path, file.bytes) {
        (Some(path), _) => {
            v.path = path.to_string_lossy().into_owned();
            v.read_file();
        }
        (None, Some(bytes)) if bytes.len() <= wavetable::MAX_IMPORT_BYTES => {
            v.start_pending(bytes.to_vec(), &file.name)
        }
        _ => v.error = Some("The dropped file could not be read".into()),
    }
}

pub fn show(editor: &mut PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    dropped(editor, ui_state, ctx);
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
        // Below the toolbar (Save, the view switch), inside the window.
        .default_pos(pos2(24.0, 56.0))
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
    } else {
        // Closed: forget the table, remember the folder.
        ui_state.tables = View {
            last_dir: v.last_dir.take(),
            ..View::default()
        };
    }
}
