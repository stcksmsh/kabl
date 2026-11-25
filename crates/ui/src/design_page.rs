//! The composite design page (docs/composites/FACES.md section 7): one composite's face edited
//! as a draft with its own undo history, written to the project by Apply as one undo step.
use crate::kit::{self, g4, g4::Sev, Tone};
use crate::style::{Role, Style};
use crate::wheel::OwnedScroll;
use crate::{composites, panels, PatchEditor, UiState};
use egui::{pos2, vec2, Id, Pos2, Rect, Sense, Stroke};
use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement, HEIGHT},
    Composite, CompositeId, PatchState, Snapshot,
};
use kabl_modules::Taper;
use std::collections::btree_map;
use std::path::Path;

/// Every move and resize lands on this grid, in rack points.
pub const SNAP: f32 = 6.0;
/// Page undo entries kept.
const HISTORY: usize = 100;
const BAR_H: f32 = 56.0;
const PALETTE_W: f32 = 260.0;
const INSPECTOR_W: f32 = 280.0;

/// What the page edits, and the composite it was taken from.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// The composite as it was when the page opened or last reloaded.
    pub base: Composite,
    pub panel: Panel,
    pub snapshots: Vec<Snapshot>,
    pub next_interface: u64,
}

/// A draft in the page history. Its artwork carries no bytes: `art` holds each image's index in
/// the page's pool, in `arts` order.
struct Entry {
    panel: Panel,
    art: Vec<usize>,
    snapshots: Vec<Snapshot>,
    next_interface: u64,
}

/// The question the page is asking before it drops an unapplied draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    Close,
    Reload,
}

/// A move (no corner) or a resize from one corner, `(right, bottom)`.
struct Drag {
    key: u64,
    from: [f32; 4],
    grab: Pos2,
    corner: Option<(bool, bool)>,
}

pub struct Page {
    pub id: CompositeId,
    pub draft: Draft,
    pub selected: Option<u64>,
    /// The footprint of the last refused drop, drawn red until the next gesture.
    pub refused: Option<(u64, [f32; 4])>,
    /// Why the last edit was refused, or what Apply said.
    pub note: Option<String>,
    pub ask: Option<Ask>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// Each distinct PNG of this session once, so history entries stay small.
    pool: Vec<Vec<u8>>,
    drag: Option<Drag>,
}

fn rect_of(p: &Placement) -> [f32; 4] {
    [p.x, p.y, p.width, p.height]
}

fn snap(v: f32) -> f32 {
    (v / SNAP).round() * SNAP
}

/// The footprint `from` (x, y, w, h) dragged by `d` rack points: moved, or resized from
/// `corner`, with every edge that moves snapped to the grid.
pub fn dragged(from: [f32; 4], d: egui::Vec2, corner: Option<(bool, bool)>) -> [f32; 4] {
    let [x, y, w, h] = from;
    let Some((right, bottom)) = corner else {
        return [snap(x + d.x), snap(y + d.y), w, h];
    };
    let (l, r) = if right {
        (x, snap(x + w + d.x))
    } else {
        (snap(x + d.x), x + w)
    };
    let (t, b) = if bottom {
        (y, snap(y + h + d.y))
    } else {
        (snap(y + d.y), y + h)
    };
    [l, t, r - l, b - t]
}

fn arts(p: &mut Panel) -> impl Iterator<Item = &mut Artwork> {
    p.light
        .iter_mut()
        .chain(p.dark.iter_mut())
        .chain(p.placements.values_mut().flat_map(|p| p.icons.iter_mut()))
}

/// The element kind an exposure gets: a jack for a port, a selector for a stepped parameter,
/// a knob otherwise.
fn kind_for(state: &PatchState, c: &Composite, key: u64) -> Result<Kind, String> {
    if c.ports.contains_key(&key) {
        return Ok(Kind::Jack);
    }
    let e = c.controls.get(&key).ok_or("Missing exposure")?;
    Ok(
        if panels::param_info(state, &e.target)?.taper == Taper::Stepped {
            Kind::Selector
        } else {
            Kind::Knob
        },
    )
}

impl Page {
    pub fn new(id: CompositeId, c: &Composite) -> Self {
        Self {
            id,
            draft: Draft {
                base: c.clone(),
                panel: c.panel.as_deref().cloned().unwrap_or_default(),
                snapshots: c.snapshots.clone(),
                next_interface: c.next_interface,
            },
            selected: None,
            refused: None,
            note: None,
            ask: None,
            undo: Vec::new(),
            redo: Vec::new(),
            pool: Vec::new(),
            drag: None,
        }
    }

    /// The draft differs from the composite it was taken from.
    pub fn dirty(&self) -> bool {
        let b = &self.draft.base;
        let panel = match b.panel.as_deref() {
            Some(p) => *p != self.draft.panel,
            None => self.draft.panel != Panel::default(),
        };
        panel
            || self.draft.snapshots != b.snapshots
            || self.draft.next_interface != b.next_interface
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Distinct images the history refers to.
    pub fn pool_len(&self) -> usize {
        self.pool.len()
    }

    fn store(&mut self) -> Entry {
        let mut panel = self.draft.panel.clone();
        let art = arts(&mut panel)
            .map(|a| {
                let png = std::mem::take(&mut a.png);
                match self.pool.iter().position(|p| *p == png) {
                    Some(i) => i,
                    None => {
                        self.pool.push(png);
                        self.pool.len() - 1
                    }
                }
            })
            .collect();
        Entry {
            panel,
            art,
            snapshots: self.draft.snapshots.clone(),
            next_interface: self.draft.next_interface,
        }
    }

    fn load(&mut self, mut e: Entry) {
        for (a, &i) in arts(&mut e.panel).zip(&e.art) {
            a.png = self.pool[i].clone();
        }
        self.draft.panel = e.panel;
        self.draft.snapshots = e.snapshots;
        self.draft.next_interface = e.next_interface;
        self.refused = None;
        self.note = None;
        self.selected = self
            .selected
            .filter(|k| self.draft.panel.placements.contains_key(k));
    }

    /// Records the draft before an edit, as one page undo step.
    fn push(&mut self) {
        let e = self.store();
        self.undo.push(e);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.refused = None;
        self.note = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(e) = self.undo.pop() else {
            return false;
        };
        let now = self.store();
        self.redo.push(now);
        self.load(e);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(e) = self.redo.pop() else {
            return false;
        };
        let now = self.store();
        self.undo.push(now);
        self.load(e);
        true
    }

    /// Replaces the draft's panel with `panel` as one step when it is a valid face; otherwise
    /// keeps the draft and says why.
    fn commit(&mut self, state: &PatchState, panel: Panel) -> Result<(), String> {
        if panel == self.draft.panel {
            return Ok(());
        }
        if let Err(e) = panels::check(state, &self.draft.base, &panel) {
            self.note = Some(e.clone());
            return Err(e);
        }
        self.push();
        self.draft.panel = panel;
        Ok(())
    }

    /// Moves or resizes element `key` to `r` (x, y, w, h), placing it first when it is not on
    /// the face yet (a palette drop). A refused footprint is kept to be drawn red; the draft
    /// stays as it was.
    pub fn place(&mut self, state: &PatchState, key: u64, r: [f32; 4]) -> Result<(), String> {
        let mut panel = self.draft.panel.clone();
        let p = match panel.placements.entry(key) {
            btree_map::Entry::Occupied(e) => e.into_mut(),
            btree_map::Entry::Vacant(e) => e.insert(self.fresh(state, key)?),
        };
        [p.x, p.y, p.width, p.height] = r;
        let result = self.commit(state, panel);
        if result.is_err() {
            self.refused = Some((key, r));
        }
        result
    }

    /// Exposure `key` as a new element of its default kind and size, at the origin.
    fn fresh(&self, state: &PatchState, key: u64) -> Result<Placement, String> {
        let kind = kind_for(state, &self.draft.base, key)?;
        let (w, h) = match kind {
            Kind::Knob => (84.0, 110.0),
            Kind::Selector => {
                let e = &self.draft.base.controls[&key];
                let p = panels::param_info(state, &e.target)?;
                (
                    180.0f32.max(((p.max - p.min + 1.0) * 18.0 / SNAP).ceil() * SNAP),
                    55.0,
                )
            }
            _ => (64.0, 58.0),
        };
        Ok(Placement {
            kind,
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            source: None,
            label: None,
            icons: Vec::new(),
        })
    }

    /// Places exposure `key` at the first free spot of its default size, top-left first.
    pub fn add(&mut self, state: &PatchState, key: u64) -> Result<(), String> {
        if self.draft.panel.placements.contains_key(&key) {
            self.selected = Some(key);
            return Ok(());
        }
        let mut new = self.fresh(state, key)?;
        let (w, h) = (new.width, new.height);
        let panel = &self.draft.panel;
        let free = |x: f32, y: f32| {
            panel
                .placements
                .values()
                .all(|o| x >= o.x + o.width || x + w <= o.x || y >= o.y + o.height || y + h <= o.y)
        };
        let spot = (0..)
            .map(|j| 66.0 + SNAP * j as f32)
            .take_while(|y| y + h <= HEIGHT - 32.0)
            .flat_map(|y| {
                (0..)
                    .map(|i| 12.0 + SNAP * i as f32)
                    .take_while(|x| x + w <= panel.width - 8.0)
                    .map(move |x| (x, y))
            })
            .find(|&(x, y)| free(x, y));
        let Some((x, y)) = spot else {
            let e = "No free spot on the face: make it wider or move elements".to_string();
            self.note = Some(e.clone());
            return Err(e);
        };
        let mut panel = panel.clone();
        (new.x, new.y) = (x, y);
        panel.placements.insert(key, new);
        self.commit(state, panel)?;
        self.selected = Some(key);
        Ok(())
    }

    pub fn remove(&mut self, key: u64) {
        if self.draft.panel.placements.contains_key(&key) {
            self.push();
            self.draft.panel.placements.remove(&key);
            self.selected = None;
        }
    }

    pub fn set_width(&mut self, state: &PatchState, width: f32) -> Result<(), String> {
        let mut panel = self.draft.panel.clone();
        panel.width = width;
        self.commit(state, panel)
    }

    /// Sets (or clears) the light or dark face artwork.
    pub fn set_art(&mut self, dark: bool, art: Option<Artwork>) {
        let slot = if dark {
            &self.draft.panel.dark
        } else {
            &self.draft.panel.light
        };
        if *slot == art {
            return;
        }
        self.push();
        if dark {
            self.draft.panel.dark = art;
        } else {
            self.draft.panel.light = art;
        }
    }

    /// Clears both face images as one step.
    pub fn clear_art(&mut self) {
        if self.draft.panel.light.is_some() || self.draft.panel.dark.is_some() {
            self.push();
            self.draft.panel.light = None;
            self.draft.panel.dark = None;
        }
    }

    /// Starts over from the default face; Apply then writes no panel.
    pub fn default_face(&mut self) {
        if self.draft.panel != Panel::default() {
            self.push();
            self.draft.panel = Panel::default();
            self.selected = None;
        }
    }
}

/// Writes the draft over the current composite as one project undo step: its panel, snapshots
/// and `next_interface`, nothing else. Refused when the composite changed since the page took
/// its draft.
fn apply(editor: &mut PatchEditor, page: &mut Page) -> Result<(), String> {
    let current = editor
        .state()
        .composites
        .get(&page.id)
        .ok_or("The composite was removed")?;
    if *current != page.draft.base {
        return Err("The composite changed: Reload before applying".into());
    }
    let mut c = current.clone();
    c.panel = (page.draft.panel != Panel::default()).then(|| Box::new(page.draft.panel.clone()));
    c.snapshots = page.draft.snapshots.clone();
    c.next_interface = page.draft.next_interface;
    composites::set(editor, page.id, c)?;
    let selected = page.selected;
    *page = Page::new(page.id, &editor.state().composites[&page.id]);
    page.selected = selected;
    Ok(())
}

/// A number field that edits a text copy and gives the number back when focus leaves it.
fn num_field(ui: &mut egui::Ui, st: &Style, key: &str, v: f32) -> (egui::Response, Option<f32>) {
    let id = Id::new(("design-num", key));
    let mut s = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| format!("{v}"));
    let r = kit::field(ui, st, &mut s, "", None, Some(64.0));
    let mut out = None;
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, s));
    } else {
        if r.lost_focus() {
            out = s.trim().parse::<f32>().ok().filter(|x| x.is_finite());
        }
        ui.data_mut(|d| d.remove::<String>(id));
    }
    (r, out)
}

/// The page, in the rack's place: top bar, palette, canvas, inspector.
pub fn show(editor: &mut PatchEditor, view: &mut UiState, ui: &mut egui::Ui) {
    let Some(mut page) = view.panels.page.take() else {
        return;
    };
    let st = view.style.clone();
    let st = &*st;
    // Removed (an undo, say) counts as changed: the draft stays until Close or Reload asks.
    let current = editor.state().composites.get(&page.id).cloned();
    let changed = current.as_ref() != Some(&page.draft.base);
    let area = ui.max_rect();
    let bar = Rect::from_min_max(area.min, pos2(area.right(), area.top() + BAR_H));
    let left = Rect::from_min_max(
        pos2(area.left(), bar.bottom()),
        pos2(area.left() + PALETTE_W, area.bottom()),
    );
    let right = Rect::from_min_max(pos2(area.right() - INSPECTOR_W, bar.bottom()), area.max);
    let canvas = Rect::from_min_max(pos2(left.right(), bar.bottom()), right.left_bottom());
    let mut close = false;
    if page.ask.is_none() && !crate::routing::wants_text_input(ui.ctx()) {
        keys(editor, ui, &mut page, &mut close);
    }
    let p = ui.painter();
    p.rect_filled(bar, 0.0, st.roles.bar);
    p.rect_filled(left, 0.0, st.roles.surface);
    p.rect_filled(right, 0.0, st.roles.surface);
    for (a, b) in [
        (bar.left_bottom(), bar.right_bottom()),
        (left.right_top(), left.right_bottom()),
        (right.left_top(), right.left_bottom()),
    ] {
        p.line_segment([a, b], Stroke::new(1.0, st.roles.line));
    }
    top_bar(editor, view, ui, st, &mut page, changed, bar, &mut close);
    palette(editor, view, ui, st, &mut page, left);
    face(editor, view, ui, st, &mut page, current.as_ref(), canvas);
    inspector(editor, view, ui, st, &mut page, right);
    if changed {
        let strip = Rect::from_min_size(
            canvas.min + vec2(16.0, 12.0),
            vec2(canvas.width() - 32.0, 64.0),
        );
        view.record("design:changed".into(), strip);
        ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
            g4::banner(
                ui,
                st,
                Sev::Warn,
                "The composite changed",
                Some(if current.is_some() {
                    "Reload to edit the current composite; Apply is off until then."
                } else {
                    "It was removed: Apply is off, and Reload closes the page."
                }),
            );
        });
    }
    ask(editor, view, ui.ctx(), &mut page, &mut close);
    if !close {
        view.panels.page = Some(page);
    }
}

fn keys(editor: &PatchEditor, ui: &mut egui::Ui, page: &mut Page, close: &mut bool) {
    use egui::{Key, KeyboardShortcut as S, Modifiers as M};
    let (undo, redo, esc, del, nudge) = ui.input_mut(|i| {
        let redo = i.consume_shortcut(&S::new(M::COMMAND | M::SHIFT, Key::Z))
            || i.consume_shortcut(&S::new(M::COMMAND, Key::Y));
        let undo = i.consume_shortcut(&S::new(M::COMMAND, Key::Z));
        let nudge = [
            (Key::ArrowLeft, -SNAP, 0.0),
            (Key::ArrowRight, SNAP, 0.0),
            (Key::ArrowUp, 0.0, -SNAP),
            (Key::ArrowDown, 0.0, SNAP),
        ]
        .into_iter()
        .find(|(k, ..)| i.consume_key(M::NONE, *k))
        .map(|(_, x, y)| vec2(x, y));
        (
            undo,
            redo,
            i.consume_key(M::NONE, Key::Escape),
            i.consume_key(M::NONE, Key::Delete),
            nudge,
        )
    });
    if undo {
        page.undo();
    }
    if redo {
        page.redo();
    }
    // Esc cancels a drag in flight before it closes the page.
    if esc && page.drag.take().is_none() {
        close_or_ask(page, close);
    }
    let Some(key) = page.selected else { return };
    if del {
        page.remove(key);
    }
    if let (Some(d), Some(p)) = (nudge, page.draft.panel.placements.get(&key)) {
        let [x, y, w, h] = rect_of(p);
        // A refusal keeps the draft and shows its reason in `page.note`.
        let _ = page.place(editor.state(), key, [x + d.x, y + d.y, w, h]);
    }
}

fn close_or_ask(page: &mut Page, close: &mut bool) {
    if page.dirty() {
        page.ask = Some(Ask::Close);
    } else {
        *close = true;
    }
}

/// Starts over from the current composite, or closes the page when it was removed.
fn reload(editor: &PatchEditor, page: &mut Page, close: &mut bool) {
    match editor.state().composites.get(&page.id) {
        Some(c) => *page = Page::new(page.id, c),
        None => *close = true,
    }
}

fn button(
    ui: &mut egui::Ui,
    view: &mut UiState,
    st: &Style,
    key: &str,
    text: &str,
    on: bool,
) -> bool {
    let r = ui.add(kit::Button::new(st, text).g4(32.0).ghost().enabled(on));
    view.record(format!("design:{key}"), r.rect);
    r.clicked()
}

#[allow(clippy::too_many_arguments)]
fn top_bar(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    page: &mut Page,
    changed: bool,
    rect: Rect,
    close: &mut bool,
) {
    let inner = rect.shrink2(vec2(16.0, 12.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let c = &page.draft.base;
            kit::label(
                ui,
                st,
                Role::H3,
                Tone::Text,
                format!("{} · #{}", c.name, page.id),
            );
            ui.add_space(8.0);
            let w = page.draft.panel.width;
            if button(ui, view, st, "width-down", "−", w > 180.0) {
                // A refusal keeps the draft and shows its reason in `page.note`.
                let _ = page.set_width(editor.state(), w - 30.0);
            }
            kit::label(ui, st, Role::Value, Tone::Text2, format!("{w} pt"));
            if button(ui, view, st, "width-up", "+", w < 840.0) {
                // A refusal keeps the draft and shows its reason in `page.note`.
                let _ = page.set_width(editor.state(), w + 30.0);
            }
            kit::segmented(ui, st, |ui| {
                for (key, text, dark) in [("light", "Light", false), ("dark", "Dark", true)] {
                    let r = kit::seg(ui, st, text, view.dark == dark);
                    view.record(format!("design:{key}"), r.rect);
                    if r.clicked() {
                        view.dark = dark;
                    }
                }
            });
            if button(ui, view, st, "undo", "Undo", page.can_undo()) {
                page.undo();
            }
            if button(ui, view, st, "redo", "Redo", page.can_redo()) {
                page.redo();
            }
            if button(ui, view, st, "reload", "Reload", true) {
                if page.dirty() {
                    page.ask = Some(Ask::Reload);
                } else {
                    reload(editor, page, close);
                }
            }
            if button(ui, view, st, "default", "Default face", true) {
                page.default_face();
            }
            if button(ui, view, st, "export", "Export…", true) {
                page.note = panels::export(editor, page.id, Path::new(&view.panels.path)).err();
            }
            if button(ui, view, st, "import-package", "Import…", true) {
                let result = panels::read_package(Path::new(&view.panels.path))
                    .and_then(|pkg| composites::insert(editor, &pkg));
                match result {
                    Ok(id) => {
                        view.last_message = Some(format!("Imported independent instance #{id}"))
                    }
                    Err(e) => page.note = Some(e),
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, view, st, "close", "Close", true) {
                    close_or_ask(page, close);
                }
                let on = page.dirty() && !changed;
                let r = ui.add(kit::Button::new(st, "Apply").g4(32.0).primary().enabled(on));
                view.record("design:apply".into(), r.rect);
                if r.clicked() {
                    page.note = apply(editor, page).err();
                }
            });
        },
    );
}

fn palette(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    page: &mut Page,
    rect: Rect,
) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        kit::section(ui, st, "Controls and jacks");
        let c = page.draft.base.clone();
        egui::ScrollArea::vertical()
            .id_salt("design-palette")
            .show_owned(ui, |ui| {
                for (&key, e) in c.controls.iter().chain(&c.ports) {
                    let kind = kind_for(editor.state(), &c, key)
                        .map(|k| format!("{k:?}"))
                        .unwrap_or_else(|e| e);
                    let placed = page.draft.panel.placements.contains_key(&key);
                    let fresh = (!placed)
                        .then(|| page.fresh(editor.state(), key).ok())
                        .flatten();
                    ui.horizontal(|ui| {
                        let mut r = ui.add(
                            kit::Button::new(st, format!("{kind} · {}", e.label))
                                .g4(32.0)
                                .ghost()
                                .min_width(if placed { 160.0 } else { 236.0 }),
                        );
                        view.record(format!("design:palette:{key}"), r.rect);
                        if placed {
                            g4::chip_label(ui, st, "placed", true, Some(st.roles.good));
                        }
                        // Only an exposure that can still be placed drags onto the face, held by
                        // its middle.
                        if let Some(p) = &fresh {
                            r = r.interact(Sense::drag());
                            if r.drag_started() {
                                page.drag = Some(Drag {
                                    key,
                                    from: rect_of(p),
                                    grab: pos2(p.width / 2.0, p.height / 2.0),
                                    corner: None,
                                });
                                page.refused = None;
                                page.note = None;
                            }
                        }
                        if r.clicked() {
                            // A refusal keeps the draft and shows its reason in `page.note`.
                            let _ = page.add(editor.state(), key);
                        }
                    });
                }
            });
    });
}

fn inspector(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    page: &mut Page,
    rect: Rect,
) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        if let Some(n) = page.note.clone() {
            g4::banner(ui, st, Sev::Bad, "Not applied", Some(&n));
        }
        let selected = page
            .selected
            .and_then(|k| Some((k, page.draft.panel.placements.get(&k)?.clone())));
        if let Some((key, p)) = selected {
            kit::section(ui, st, "Element");
            let label = page
                .draft
                .base
                .controls
                .get(&key)
                .or_else(|| page.draft.base.ports.get(&key))
                .map(|e| e.label.clone())
                .unwrap_or_default();
            kit::label(
                ui,
                st,
                Role::Body,
                Tone::Text,
                format!("{:?} · {label}", p.kind),
            );
            let mut r = rect_of(&p);
            let mut commit = false;
            for row in [[0, 1], [2, 3]] {
                ui.horizontal(|ui| {
                    for i in row {
                        let name = ["x", "y", "w", "h"][i];
                        kit::label(ui, st, Role::Caption, Tone::Text3, name);
                        let (resp, v) = num_field(ui, st, &format!("{key}-{name}"), r[i]);
                        view.record(format!("design:{name}"), resp.rect);
                        if let Some(v) = v {
                            r[i] = v;
                            commit = true;
                        }
                    }
                });
            }
            if commit {
                // A refusal keeps the draft and shows its reason in `page.note`.
                let _ = page.place(editor.state(), key, r);
            }
            if button(ui, view, st, "remove", "Remove from face", true) {
                page.remove(key);
            }
            return;
        }
        kit::section(ui, st, "Face artwork");
        let r = kit::field(ui, st, &mut view.panels.path, "Local file path", None, None);
        view.record("design:path".into(), r.rect);
        if button(ui, view, st, "browse", "Browse…", true) {
            let hint = view.panels.path.clone();
            crate::filebrowser::open(
                view,
                crate::filebrowser::Purpose::PanelArt,
                "Choose artwork",
                crate::filebrowser::Mode::Open,
                crate::filebrowser::IMAGE,
                "",
                &hint,
            );
        }
        for (dark, key, text) in [
            (false, "import-light", "Light PNG"),
            (true, "import-dark", "Dark PNG"),
        ] {
            let have = if dark {
                &page.draft.panel.dark
            } else {
                &page.draft.panel.light
            };
            let name = have.as_ref().map(|a| a.name.clone());
            ui.horizontal(|ui| {
                kit::label(ui, st, Role::Body, Tone::Text2, text);
                let dot = if name.is_some() {
                    st.roles.good
                } else {
                    st.roles.warn
                };
                g4::chip_label(
                    ui,
                    st,
                    name.as_deref().unwrap_or("missing"),
                    true,
                    Some(dot),
                );
            });
            if button(ui, view, st, key, "Import…", true) {
                match panels::import_art(Path::new(&view.panels.path)) {
                    Ok(a) => page.set_art(dark, Some(a)),
                    Err(e) => page.note = Some(e),
                }
            }
        }
        let any = page.draft.panel.light.is_some() || page.draft.panel.dark.is_some();
        if button(ui, view, st, "clear-art", "Clear artwork", any) {
            page.clear_art();
        }
    });
}

/// The face drawn from the draft (with a drag in flight applied), the grid and bands, and the
/// footprint handles that take all input in place of the face's own controls.
fn face(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    page: &mut Page,
    current: Option<&Composite>,
    rect: Rect,
) {
    let mut panel = page.draft.panel.clone();
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let z = ((rect.width() - 64.0) / panel.width)
        .min((rect.height() - 160.0) / HEIGHT)
        .clamp(0.25, 3.0);
    let size = vec2(panel.width, HEIGHT) * z;
    let xf = crate::Xf {
        origin: rect.center() - size / 2.0,
        zoom: z,
    };
    // A drag in flight: where the element would land, and why it may not.
    let mut live = None;
    if let (Some(d), Some(at)) = (&page.drag, pointer) {
        let to = dragged(d.from, xf.inv(at) - d.grab, d.corner);
        // A palette drag shows its element only over the canvas.
        if !panel.placements.contains_key(&d.key) && rect.contains(at) {
            if let Ok(p) = page.fresh(editor.state(), d.key) {
                panel.placements.insert(d.key, p);
            }
        }
        if let Some(p) = panel.placements.get_mut(&d.key) {
            [p.x, p.y, p.width, p.height] = to;
            live = Some((
                d.key,
                to,
                panels::check(editor.state(), &page.draft.base, &panel),
            ));
        }
    }
    let world = Rect::from_min_size(Pos2::ZERO, vec2(panel.width, HEIGHT));
    let painter = ui.painter_at(rect);
    if let Some(current) = current {
        let mut shown = current.clone();
        shown.panel = Some(Box::new(panel.clone()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        // The face's own hit rects would point the rack's keys at the page.
        let hits = std::mem::take(&mut view.frame_hits);
        crate::draw_authored_face(
            editor,
            view,
            &mut child,
            xf,
            &mut crate::Drawn::default(),
            page.id,
            &shown,
            world,
        );
        view.frame_hits = hits;
    } else {
        // The composite is gone, and its modules may be too: footprints only.
        painter.rect_filled(xf.r(world), 2.0, st.roles.rack);
        for p in panel.placements.values() {
            let r = xf.r(Rect::from_min_size(pos2(p.x, p.y), vec2(p.width, p.height)));
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, st.roles.line2),
                egui::StrokeKind::Inside,
            );
        }
    }
    let band = st.roles.accent.gamma_multiply(0.12);
    for (y, h) in [(0.0, 64.0), (HEIGHT - 32.0, 32.0)] {
        let r = xf.r(Rect::from_min_size(pos2(0.0, y), vec2(panel.width, h)));
        painter.rect_filled(r, 0.0, band);
    }
    let grid = |i: usize| {
        let a = if i.is_multiple_of(5) { 0.35 } else { 0.12 };
        Stroke::new(1.0, st.roles.line2.gamma_multiply(a))
    };
    let face = xf.r(world);
    for (i, x) in (0..=(panel.width / SNAP) as usize).map(|i| (i, i as f32 * SNAP)) {
        let sx = xf.p(pos2(x, 0.0)).x;
        painter.line_segment([pos2(sx, face.top()), pos2(sx, face.bottom())], grid(i));
    }
    for (i, y) in (0..=(HEIGHT / SNAP) as usize).map(|i| (i, i as f32 * SNAP)) {
        let sy = xf.p(pos2(0.0, y)).y;
        painter.line_segment([pos2(face.left(), sy), pos2(face.right(), sy)], grid(i));
    }
    let bg = ui.interact(rect, Id::new("design-canvas"), Sense::click());
    if bg.clicked() {
        page.selected = None;
        page.refused = None;
        page.note = None;
    }
    let state = editor.state();
    let gesture = |page: &mut Page, resp: &egui::Response, key: u64, corner| {
        if resp.drag_started() {
            if let (Some(p), Some(at)) = (
                page.draft.panel.placements.get(&key),
                ui.input(|i| i.pointer.press_origin()),
            ) {
                page.drag = Some(Drag {
                    key,
                    from: rect_of(p),
                    grab: xf.inv(at),
                    corner,
                });
            }
            page.selected = Some(key);
            page.refused = None;
            page.note = None;
        }
        if resp.drag_stopped() {
            if let (Some(d), Some(at)) = (page.drag.take(), pointer) {
                let to = dragged(d.from, xf.inv(at) - d.grab, d.corner);
                // A refusal keeps the draft and shows its reason in `page.note`.
                let _ = page.place(state, d.key, to);
            }
        }
        if resp.clicked() {
            page.selected = Some(key);
        }
    };
    // A palette drag's element is not on the draft yet: it takes no input.
    let ghost = page
        .drag
        .as_ref()
        .map(|d| d.key)
        .filter(|k| !page.draft.panel.placements.contains_key(k));
    for (&key, p) in panel.placements.iter().filter(|(&k, _)| Some(k) != ghost) {
        let r = xf.r(Rect::from_min_size(pos2(p.x, p.y), vec2(p.width, p.height)));
        let resp = ui.interact(r, Id::new(("design-el", key)), Sense::click_and_drag());
        view.record(format!("design:el:{key}"), r);
        let selected = page.selected == Some(key);
        if selected || resp.hovered() {
            let c = if selected {
                st.roles.accent
            } else {
                st.roles.line2
            };
            painter.rect_stroke(r, 2.0, Stroke::new(1.5, c), egui::StrokeKind::Outside);
        }
        gesture(page, &resp, key, None);
    }
    if let Some(p) = page
        .selected
        .and_then(|k| Some((k, panel.placements.get(&k)?)))
    {
        let (key, p) = p;
        let r = xf.r(Rect::from_min_size(pos2(p.x, p.y), vec2(p.width, p.height)));
        for (name, right, bottom) in [
            ("nw", false, false),
            ("ne", true, false),
            ("sw", false, true),
            ("se", true, true),
        ] {
            let at = pos2(
                if right { r.right() } else { r.left() },
                if bottom { r.bottom() } else { r.top() },
            );
            let h = Rect::from_center_size(at, vec2(12.0, 12.0));
            painter.rect_filled(h.shrink(2.0), 1.0, st.roles.accent);
            let resp = ui.interact(h, Id::new(("design-corner", key, name)), Sense::drag());
            view.record(format!("design:corner:{key}:{name}"), h);
            gesture(page, &resp, key, Some((right, bottom)));
        }
    }
    // Refused, in flight or just dropped: red, with the reason.
    let red = match live {
        Some((key, to, Err(e))) => Some((key, to, e)),
        Some(_) => None,
        None => page
            .refused
            .map(|(k, to)| (k, to, page.note.clone().unwrap_or_default())),
    };
    if let Some((_, [x, y, w, h], why)) = red {
        let r = xf.r(Rect::from_min_size(pos2(x, y), vec2(w, h)));
        painter.rect_filled(r, 2.0, st.roles.bad.gamma_multiply(0.25));
        painter.rect_stroke(
            r,
            2.0,
            Stroke::new(2.0, st.roles.bad),
            egui::StrokeKind::Outside,
        );
        painter.text(
            pos2(face.left(), face.bottom() + 12.0),
            egui::Align2::LEFT_TOP,
            why,
            st.font(Role::Caption),
            st.roles.bad,
        );
    }
    // A palette drag drops where it is released over the canvas; anywhere else cancels it.
    let fresh = page
        .drag
        .as_ref()
        .is_some_and(|d| !page.draft.panel.placements.contains_key(&d.key));
    if fresh && ui.input(|i| i.pointer.any_released()) {
        if let (Some(d), Some(at)) = (page.drag.take(), pointer) {
            if rect.contains(at) {
                let to = dragged(d.from, xf.inv(at) - d.grab, None);
                // A refusal keeps the draft and shows its reason in `page.note`.
                if page.place(state, d.key, to).is_ok() {
                    page.selected = Some(d.key);
                }
            }
        }
    }
    if !ui.input(|i| i.pointer.any_down()) {
        page.drag = None;
    }
}

fn ask(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ctx: &egui::Context,
    page: &mut Page,
    close: &mut bool,
) {
    let Some(q) = page.ask else { return };
    let st = view.style.clone();
    let st = &*st;
    let (title, detail) = match q {
        Ask::Close => (
            "Apply the face before closing?",
            "The draft has changes that are not applied.",
        ),
        Ask::Reload => (
            "Discard the draft and reload?",
            "The draft's changes are lost.",
        ),
    };
    egui::Modal::new(Id::new("design-ask")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
        kit::label(ui, st, Role::H3, Tone::Text, title);
        kit::paragraph(ui, st, Role::Body, Tone::Text2, detail);
        ui.horizontal(|ui| {
            if q == Ask::Close && button(ui, view, st, "ask:apply", "Apply", true) {
                page.ask = None;
                match apply(editor, page) {
                    Ok(()) => *close = true,
                    Err(e) => page.note = Some(e),
                }
            }
            let discard = if q == Ask::Close {
                "Discard"
            } else {
                "Discard and reload"
            };
            if button(ui, view, st, "ask:discard", discard, true) {
                page.ask = None;
                match q {
                    Ask::Close => *close = true,
                    Ask::Reload => reload(editor, page, close),
                }
            }
            if button(ui, view, st, "ask:keep", "Keep editing", true) {
                page.ask = None;
            }
        });
    });
}
