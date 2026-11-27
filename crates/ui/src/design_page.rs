//! The composite design page (docs/composites/FACES.md section 7): one composite's face edited
//! as a draft with its own undo history, written to the project by Apply as one undo step.
use crate::kit::{self, g4, g4::Sev, Tone};
use crate::style::{Role, Style};
use crate::wheel::OwnedScroll;
use crate::{composites, panels, PatchEditor, UiState};
use egui::{pos2, vec2, Id, Pos2, Rect, Sense, Stroke};
use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement, Source, HEIGHT},
    Composite, CompositeId, ModuleId, PatchState, PortRef, Snapshot,
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
/// The least footprint a slider gets: wide enough for the horizontal variant and tall enough for
/// the vertical one, since which draws is a view setting.
const SLIDER: (f32, f32) = (120.0, 168.0);
/// A new display's footprint.
const DISPLAY: (f32, f32) = (168.0, 96.0);
/// A new button's footprint: room for a cue name at the pad's type size.
const BUTTON: (f32, f32) = (132.0, 48.0);

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
    /// Apply would drop `Page::dropping`: their targets in nested composites are gone.
    Drop,
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
    /// The elements Apply would drop, with the names the page lists them by, while it asks.
    pub dropping: Vec<(u64, String)>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// Each distinct PNG of this session once, so history entries stay small.
    pool: Vec<Vec<u8>>,
    drag: Option<Drag>,
    /// Icon PNGs imported for element `.0`'s options while some option still has none.
    staged: Option<(u64, Vec<Option<Artwork>>)>,
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

/// The leaves of composite `id` whose kind has a display, in module order, each with its name:
/// `Module #N`, after the names of the composites between it and `id` (`Composite / Module #N`).
pub fn displays(state: &PatchState, id: CompositeId) -> Vec<(ModuleId, String)> {
    kabl_core::composite::leaves(state, id)
        .into_iter()
        .filter_map(|leaf| {
            let kind = &state.modules.get(&leaf)?.kind;
            if !crate::displays::has_display(kind) {
                return None;
            }
            Some((leaf, leaf_name(state, id, leaf)))
        })
        .collect()
}

/// Leaf `leaf`'s name seen from composite `id`: `Module #N`, after the composites between.
fn leaf_name(state: &PatchState, id: CompositeId, leaf: ModuleId) -> String {
    let name = state
        .modules
        .get(&leaf)
        .map(|m| crate::help::module_title(&m.kind, leaf))
        .unwrap_or_default();
    let at = state
        .composites
        .iter()
        .find(|(_, c)| c.members.contains(&leaf))
        .map(|(&c, _)| c);
    within(state, id, at, name)
}

/// `name` after the names of composite `at` and its parents up to, not including, `id`.
fn within(
    state: &PatchState,
    id: CompositeId,
    mut at: Option<CompositeId>,
    mut name: String,
) -> String {
    while let Some(c) = at
        .filter(|&c| c != id)
        .and_then(|c| state.composites.get(&c))
    {
        name = format!("{} / {name}", c.name);
        at = c.parent;
    }
    name
}

/// What a button on composite `id`'s face may press, each with its name: every leaf's
/// `perform::button_actions` in module order (`Module #N · Launch Intro`), then every button on
/// the face of a nested composite (`Composite / Launch Intro`).
pub fn buttons(state: &PatchState, id: CompositeId) -> Vec<(Source, String)> {
    let actions = kabl_core::composite::leaves(state, id)
        .into_iter()
        .flat_map(|leaf| {
            let module = leaf_name(state, id, leaf);
            crate::perform::button_actions(state, leaf)
                .into_iter()
                .map(move |(action, label)| {
                    let source = Source::Module {
                        id: leaf,
                        action: Some(action),
                    };
                    (source, format!("{module} · {label}"))
                })
        });
    let faces = kabl_core::composite::descendants(state, id)
        .into_iter()
        .filter(|&d| d != id)
        .flat_map(|d| {
            state.composites[&d]
                .panel
                .iter()
                .flat_map(|p| &p.placements)
                .filter(|(_, p)| p.kind == Kind::Button)
                .map(move |(&key, p)| {
                    let name = within(state, id, Some(d), panels::button_name(state, p));
                    (Source::Face { composite: d, key }, name)
                })
        });
    actions.chain(faces).collect()
}

/// The palette hit-rect key of a button source.
fn button_hit(source: &Source) -> String {
    match source {
        Source::Module { id, action } => {
            format!("design:button:{id}.{}", action.as_deref().unwrap_or(""))
        }
        Source::Face { composite, key } => format!("design:button:face:{composite}.{key}"),
        Source::Snapshot { id } => format!("design:button:snapshot:{id}"),
    }
}

/// The kinds exposure `key` may take on the face (FACES.md section 2), its default first.
fn kinds_for(state: &PatchState, c: &Composite, key: u64) -> Result<Vec<Kind>, String> {
    Ok(match kind_for(state, c, key)? {
        // A route's amount has only the knob (`panels::check`).
        Kind::Knob
            if c.controls
                .get(&key)
                .is_some_and(|e| matches!(e.target, PortRef::CableParam { .. })) =>
        {
            vec![Kind::Knob]
        }
        Kind::Knob => vec![Kind::Knob, Kind::Slider],
        Kind::Selector => {
            let e = &c.controls[&key];
            let p = panels::param_info(state, &e.target)?;
            let mut kinds = vec![Kind::Selector];
            if p.max - p.min == 1.0 {
                kinds.push(Kind::Toggle);
            }
            // Without built-in glyphs an element becomes icons by importing a PNG per option.
            if panels::glyphs(state, &e.target) {
                kinds.push(Kind::Icons);
            }
            kinds
        }
        k => vec![k],
    })
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
            dropping: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            pool: Vec::new(),
            drag: None,
            staged: None,
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
        // Staged icons belong to the draft they were imported on.
        self.staged = None;
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

    /// Replaces the draft's panel with `panel` and its `next_interface` with `next` as one step
    /// when that is a valid face; otherwise keeps the draft and says why.
    fn commit(&mut self, state: &PatchState, panel: Panel, next: u64) -> Result<(), String> {
        if panel == self.draft.panel && next == self.draft.next_interface {
            return Ok(());
        }
        let mut c = self.draft.base.clone();
        c.next_interface = next;
        if let Err(e) = panels::check(state, &c, &panel) {
            self.note = Some(e.clone());
            return Err(e);
        }
        self.push();
        self.draft.panel = panel;
        self.draft.next_interface = next;
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
        let result = self.commit(state, panel, self.draft.next_interface);
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
        let new = self.fresh(state, key)?;
        self.put(state, key, new, self.draft.next_interface)
    }

    /// Places leaf `leaf`'s display under a fresh key at the first free spot; selects the
    /// element already showing it instead, if there is one.
    pub fn add_display(&mut self, state: &PatchState, leaf: ModuleId) -> Result<(), String> {
        if let Some(key) = self.display_of(leaf) {
            self.selected = Some(key);
            return Ok(());
        }
        let key = self.draft.next_interface;
        let new = Placement {
            kind: Kind::Display,
            x: 0.0,
            y: 0.0,
            width: DISPLAY.0,
            height: DISPLAY.1,
            source: Some(Source::Module {
                id: leaf,
                action: None,
            }),
            label: None,
            icons: Vec::new(),
        };
        self.put(state, key, new, key + 1)
    }

    /// The element showing leaf `leaf`'s display.
    pub fn display_of(&self, leaf: ModuleId) -> Option<u64> {
        self.draft.panel.placements.iter().find_map(|(&k, p)| {
            (p.kind == Kind::Display
                && p.source
                    == Some(Source::Module {
                        id: leaf,
                        action: None,
                    }))
            .then_some(k)
        })
    }

    /// Places a button pressing `source` under a fresh key at the first free spot; selects the
    /// element already pressing it instead, if there is one.
    pub fn add_button(&mut self, state: &PatchState, source: Source) -> Result<(), String> {
        if let Some(key) = self.button_of(&source) {
            self.selected = Some(key);
            return Ok(());
        }
        let key = self.draft.next_interface;
        let new = Placement {
            kind: Kind::Button,
            x: 0.0,
            y: 0.0,
            width: BUTTON.0,
            height: BUTTON.1,
            source: Some(source),
            label: None,
            icons: Vec::new(),
        };
        self.put(state, key, new, key + 1)
    }

    /// The button element pressing `source`.
    pub fn button_of(&self, source: &Source) -> Option<u64> {
        self.draft.panel.placements.iter().find_map(|(&k, p)| {
            (p.kind == Kind::Button && p.source.as_ref() == Some(source)).then_some(k)
        })
    }

    /// `new` as element `key` at the first free spot of its size, top-left first, with the
    /// draft's `next_interface` becoming `next`.
    fn put(
        &mut self,
        state: &PatchState,
        key: u64,
        mut new: Placement,
        next: u64,
    ) -> Result<(), String> {
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
        self.commit(state, panel, next)?;
        self.selected = Some(key);
        Ok(())
    }

    /// Draws placed element `key` as `kind`, from the same corner; a slider grows to fit either
    /// variant.
    pub fn set_kind(&mut self, state: &PatchState, key: u64, kind: Kind) -> Result<(), String> {
        let mut panel = self.draft.panel.clone();
        let p = panel.placements.get_mut(&key).ok_or("Not on the face")?;
        p.kind = kind;
        if kind != Kind::Icons {
            p.icons.clear();
        }
        if kind == Kind::Slider {
            p.width = p.width.max(SLIDER.0);
            p.height = p.height.max(SLIDER.1);
        }
        self.commit(state, panel, self.draft.next_interface)?;
        self.staged = self.staged.take().filter(|(k, _)| *k != key);
        Ok(())
    }

    /// The option count of stepped control `key`, None for any other exposure.
    pub fn options(&self, state: &PatchState, key: u64) -> Option<usize> {
        let e = self.draft.base.controls.get(&key)?;
        let p = panels::param_info(state, &e.target).ok()?;
        (p.taper == Taper::Stepped).then(|| (p.max - p.min).round() as usize + 1)
    }

    /// Element `key`'s icon PNG per option: the draft's, or those imported so far.
    pub fn icons(&self, state: &PatchState, key: u64) -> Vec<Option<Artwork>> {
        let n = self.options(state, key).unwrap_or(0);
        match (&self.staged, self.draft.panel.placements.get(&key)) {
            (Some((k, v)), _) if *k == key && v.len() == n => v.clone(),
            (_, Some(p)) if p.icons.len() == n => p.icons.iter().cloned().map(Some).collect(),
            _ => vec![None; n],
        }
    }

    /// Sets option `k`'s icon PNG of stepped element `key`. Once every option has one, the
    /// element becomes an icon selector of those PNGs as one step; until then they wait here.
    pub fn set_icon(
        &mut self,
        state: &PatchState,
        key: u64,
        k: usize,
        art: Artwork,
    ) -> Result<(), String> {
        let mut icons = self.icons(state, key);
        let checked = art
            .validate()
            .and_then(|()| match self.draft.panel.placements.get(&key) {
                Some(_) if k < icons.len() => Ok(()),
                Some(_) => Err("Icons need a stepped control on the face".to_string()),
                None => Err("Not on the face".to_string()),
            });
        if let Err(e) = checked {
            self.note = Some(e.clone());
            return Err(e);
        }
        icons[k] = Some(art);
        if icons.iter().any(Option::is_none) {
            self.staged = Some((key, icons));
            return Ok(());
        }
        let mut panel = self.draft.panel.clone();
        let p = panel.placements.get_mut(&key).expect("checked above");
        p.kind = Kind::Icons;
        p.icons = icons.iter().flatten().cloned().collect();
        let result = self.commit(state, panel, self.draft.next_interface);
        self.staged = result.is_err().then_some((key, icons));
        result
    }

    pub fn remove(&mut self, key: u64) {
        if self.draft.panel.placements.contains_key(&key) {
            self.push();
            self.draft.panel.placements.remove(&key);
            self.selected = None;
            self.staged = self.staged.take().filter(|(k, _)| *k != key);
        }
    }

    pub fn set_width(&mut self, state: &PatchState, width: f32) -> Result<(), String> {
        let mut panel = self.draft.panel.clone();
        panel.width = width;
        self.commit(state, panel, self.draft.next_interface)
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
/// its draft. When nested composites lost what some elements press, it writes nothing, lists
/// those elements in `page.dropping` and asks, unless `confirmed`, which writes the draft without
/// them. Whether it wrote.
fn apply(editor: &mut PatchEditor, page: &mut Page, confirmed: bool) -> Result<bool, String> {
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
    let mut candidate = editor.state().clone();
    candidate.composites.insert(page.id, c.clone());
    // The document is pruned after every edit, so only the written composite can change.
    if let Some(pruned) = kabl_core::composite::prune(&candidate)
        .into_iter()
        .find_map(|op| match op {
            kabl_core::Op::SetComposite { id, value } if id == page.id => value,
            _ => None,
        })
    {
        let kept = pruned.panel.as_ref().map(|p| &p.placements);
        let dropping: Vec<(u64, String)> = page
            .draft
            .panel
            .placements
            .iter()
            .filter(|(k, _)| kept.is_none_or(|kept| !kept.contains_key(k)))
            .map(|(&k, p)| {
                let name = p.label.clone().unwrap_or_else(|| format!("{:?}", p.kind));
                (k, format!("{name} · element {k}"))
            })
            .collect();
        if !dropping.is_empty() && !confirmed {
            page.dropping = dropping;
            page.ask = Some(Ask::Drop);
            return Ok(false);
        }
        c = pruned;
    }
    composites::set(editor, page.id, c)?;
    let selected = page.selected;
    *page = Page::new(page.id, &editor.state().composites[&page.id]);
    page.selected = selected;
    Ok(true)
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

/// The local file path the Import buttons read, with Browse.
fn path_field(ui: &mut egui::Ui, view: &mut UiState, st: &Style) {
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
                    page.note = apply(editor, page, false).err();
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
                kit::section(ui, st, "Displays");
                for (leaf, name) in displays(editor.state(), page.id) {
                    let placed = page.display_of(leaf).is_some();
                    ui.horizontal(|ui| {
                        let r = ui.add(
                            kit::Button::new(st, name)
                                .g4(32.0)
                                .ghost()
                                .min_width(if placed { 160.0 } else { 236.0 }),
                        );
                        view.record(format!("design:display:{leaf}"), r.rect);
                        if placed {
                            g4::chip_label(ui, st, "placed", true, Some(st.roles.good));
                        }
                        if r.clicked() {
                            // A refusal keeps the draft and shows its reason in `page.note`.
                            let _ = page.add_display(editor.state(), leaf);
                        }
                    });
                }
                kit::section(ui, st, "Buttons");
                for (source, name) in buttons(editor.state(), page.id) {
                    let placed = page.button_of(&source).is_some();
                    ui.horizontal(|ui| {
                        let r = ui.add(
                            kit::Button::new(st, name)
                                .g4(32.0)
                                .ghost()
                                .min_width(if placed { 160.0 } else { 236.0 }),
                        );
                        view.record(button_hit(&source), r.rect);
                        if placed {
                            g4::chip_label(ui, st, "placed", true, Some(st.roles.good));
                        }
                        if r.clicked() {
                            // A refusal keeps the draft and shows why.
                            page.note = page.add_button(editor.state(), source).err();
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
                .or_else(|| match &p.source {
                    Some(Source::Module { id: leaf, .. }) if p.kind == Kind::Display => {
                        displays(editor.state(), page.id)
                            .into_iter()
                            .find_map(|(m, name)| (m == *leaf).then_some(name))
                    }
                    Some(source) if p.kind == Kind::Button => buttons(editor.state(), page.id)
                        .into_iter()
                        .find_map(|(s, name)| (s == *source).then_some(name)),
                    _ => None,
                })
                .unwrap_or_default();
            kit::label(
                ui,
                st,
                Role::Body,
                Tone::Text,
                format!("{:?} · {label}", p.kind),
            );
            let mut kinds = kinds_for(editor.state(), &page.draft.base, key).unwrap_or_default();
            // Icons of imported PNGs, on a parameter with no built-in glyphs.
            if p.kind == Kind::Icons && !kinds.contains(&Kind::Icons) {
                kinds.push(Kind::Icons);
            }
            if kinds.len() > 1 {
                let mut pick = None;
                kit::segmented(ui, st, |ui| {
                    for k in kinds {
                        let r = kit::seg(ui, st, &format!("{k:?}"), p.kind == k);
                        view.record(format!("design:kind:{k:?}"), r.rect);
                        if r.clicked() && p.kind != k {
                            pick = Some(k);
                        }
                    }
                });
                if let Some(k) = pick {
                    // A refusal keeps the draft and shows its reason in `page.note`.
                    let _ = page.set_kind(editor.state(), key, k);
                }
            }
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
            let icons = page.icons(editor.state(), key);
            if !icons.is_empty() {
                kit::section(ui, st, "Icons");
                path_field(ui, view, st);
                let target = &page.draft.base.controls[&key].target;
                let labels = match target {
                    PortRef::Param { id, param } => editor
                        .state()
                        .modules
                        .get(id)
                        .and_then(|m| crate::routing::step_labels(&m.kind, param)),
                    _ => None,
                };
                for (k, art) in icons.iter().enumerate() {
                    let mut import = false;
                    ui.horizontal(|ui| {
                        let option = labels
                            .and_then(|l| l.get(k).map(|s| s.to_string()))
                            .unwrap_or_else(|| format!("{}", k + 1));
                        kit::label(ui, st, Role::Body, Tone::Text2, option);
                        let dot = if art.is_some() {
                            st.roles.good
                        } else {
                            st.roles.warn
                        };
                        let name = art.as_ref().map_or("built-in or text", |a| &a.name);
                        g4::chip_label(ui, st, name, true, Some(dot));
                        import = button(ui, view, st, &format!("icon:{k}"), "Import…", true);
                    });
                    if import {
                        let a = panels::import_art(Path::new(&view.panels.path));
                        // A refusal keeps the draft and shows its reason in `page.note`.
                        let _ = a
                            .and_then(|a| page.set_icon(editor.state(), key, k, a))
                            .map_err(|e| page.note = Some(e));
                    }
                }
            }
            if button(ui, view, st, "remove", "Remove from face", true) {
                page.remove(key);
            }
            return;
        }
        kit::section(ui, st, "Face artwork");
        path_field(ui, view, st);
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
        Ask::Drop => (
            "Apply without these elements?",
            "What they press in a nested composite is gone, so Apply drops them:",
        ),
    };
    egui::Modal::new(Id::new("design-ask")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
        kit::label(ui, st, Role::H3, Tone::Text, title);
        kit::paragraph(ui, st, Role::Body, Tone::Text2, detail);
        if q == Ask::Drop {
            for (key, name) in &page.dropping {
                let r = kit::label(ui, st, Role::Body, Tone::Text, name);
                view.record(format!("design:dropping:{key}"), r.rect);
            }
        }
        ui.horizontal(|ui| {
            let apply_text = if q == Ask::Drop {
                "Apply without them"
            } else {
                "Apply"
            };
            if q != Ask::Reload && button(ui, view, st, "ask:apply", apply_text, true) {
                page.ask = None;
                match apply(editor, page, q == Ask::Drop) {
                    Ok(applied) => *close = q == Ask::Close && applied,
                    Err(e) => page.note = Some(e),
                }
            }
            let discard = match q {
                Ask::Close => Some("Discard"),
                Ask::Reload => Some("Discard and reload"),
                Ask::Drop => None,
            };
            if let Some(discard) = discard {
                if button(ui, view, st, "ask:discard", discard, true) {
                    page.ask = None;
                    match q {
                        Ask::Reload => reload(editor, page, close),
                        _ => *close = true,
                    }
                }
            }
            if button(ui, view, st, "ask:keep", "Keep editing", true) {
                page.ask = None;
            }
        });
    });
}
