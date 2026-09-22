//! Revision-2 interactive design prototype. Isolated: no engine, no audio device, no MIDI, no
//! patch files. Signal previews are mock drawings computed from knob values.
//!
//!     cargo run -p kabl-ui --example rev2_proto --release
//!     cargo run -p kabl-ui --example rev2_proto --release -- --size 1280x800 --dark
//!     cargo run -p kabl-ui --example rev2_proto --release -- --script <file> --shots <dir>
//!
//! See docs/design/revision-2/PROTOTYPE.md for the walkthrough and comparison switches.

mod draw;
// Shared with the rev2_env_ab example; not every item is used by both.
#[allow(dead_code)]
mod envtime;
mod geom;
mod model;
mod script;

use draw::{theme, Xf};
use egui::{pos2, vec2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Vec2};
use geom::{Cables, DepthGesture, Expansion, HiddenLayout, Hit, Layout, Region, ViewGeo};
use model::{CtlRef, History, JackRef, Kind, Patch, Route, Spec};
use std::collections::HashMap;

pub enum Gesture {
    None,
    Base { ctl: CtlRef, before: Patch },
    Depth { route: usize, before: Patch },
    Wire { from: JackRef, before: Patch, moved_route: Option<Route>, unplugged: bool },
    Pan,
}

struct Press {
    hit: Option<Hit>,
    origin: Pos2,
    started: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum EntryTarget {
    Base(CtlRef),
    Amount(usize),
}

struct Entry {
    target: EntryTarget,
    text: String,
    pos: Pos2,
    focused: bool,
    error: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum PatchChoice {
    Reference,
    Crowded,
}

pub struct App {
    pub patch: Patch,
    hist: History,
    pub geo: ViewGeo,
    pub dark: bool,
    pub auto_focus: bool,
    pub labels_on_art: bool,
    pub show_hits: bool,
    pub zoom: f32,
    pub pan: Vec2,
    pub canvas: Rect,
    pub drawer: bool,
    pub selected: Option<usize>,
    pub inspect: Option<CtlRef>,
    flash: Option<(FlashTarget, f64)>,
    toast: Option<(String, f64)>,
    pub gesture: Gesture,
    press: Option<Press>,
    pub hover: Option<Hit>,
    pub pointer: Option<Pos2>,
    entry: Option<Entry>,
    ctx_target: Option<Hit>,
    pub layout: Layout,
    pub regions: Vec<Region>,
    ab_open: bool,
    ab_slow: bool,
    ab_cache: Option<(String, envtime::Render, envtime::Render, envtime::Scenario)>,
    patch_choice: PatchChoice,
    new_route: (Option<JackRef>, Option<CtlRef>),
    slider_before: Option<(usize, Patch)>,
    pub ui_rects: HashMap<String, Rect>,
    now: f64,
    script: Option<script::Script>,
}

#[derive(Clone, Copy, PartialEq)]
enum FlashTarget {
    Ctl(CtlRef),
}

impl App {
    fn new(dark: bool, script: Option<script::Script>) -> Self {
        let patch = model::reference_patch();
        let geo = ViewGeo {
            cables: Cables::All,
            hidden_layout: HiddenLayout::StableRack,
            expansion: Expansion::Push,
            depth: DepthGesture::RingBand,
            expanded: vec![false; patch.modules.len()],
            choose: None,
            wrap_w: 1400.0,
            selected_route: None,
        };
        let layout = geom::layout(&patch, &geo);
        App {
            patch,
            hist: History::default(),
            geo,
            dark,
            auto_focus: false,
            labels_on_art: false,
            show_hits: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            canvas: Rect::NOTHING,
            drawer: false,
            selected: None,
            inspect: None,
            flash: None,
            toast: None,
            gesture: Gesture::None,
            press: None,
            hover: None,
            pointer: None,
            entry: None,
            ctx_target: None,
            layout,
            regions: vec![],
            ab_open: false,
            ab_slow: false,
            ab_cache: None,
            patch_choice: PatchChoice::Reference,
            new_route: (None, None),
            slider_before: None,
            ui_rects: HashMap::new(),
            now: 0.0,
            script,
        }
    }

    pub fn xf(&self) -> Xf {
        Xf { origin: self.canvas.min + self.pan, zoom: self.zoom }
    }

    /// Auto-Focus (a comparison option) turns All into Focus on the expanded module.
    pub fn effective_cables(&self) -> (Cables, Option<usize>) {
        if self.geo.cables == Cables::All && self.auto_focus {
            if let Some(m) = self.geo.expanded.iter().position(|e| *e) {
                return (Cables::Focus, Some(m));
            }
        }
        (self.geo.cables, self.selected)
    }

    pub fn flash_on(&self, c: CtlRef) -> bool {
        matches!(self.flash, Some((FlashTarget::Ctl(f), t)) if f == c && self.now - t < 1.5)
    }

    fn relayout(&mut self) {
        self.geo.expanded.resize(self.patch.modules.len(), false);
        self.geo.wrap_w = self.canvas.width().max(400.0) / self.zoom;
        if self.geo.selected_route.is_some_and(|r| r >= self.patch.routes.len()) {
            self.geo.selected_route = None;
        }
        self.layout = geom::layout(&self.patch, &self.geo);
        self.regions = geom::regions(&self.patch, &self.geo, &self.layout);
    }

    fn commit(&mut self, before: Patch, label: String) {
        if before != self.patch {
            self.hist.push(before, label);
        }
    }

    fn edit(&mut self, label: String, f: impl FnOnce(&mut Patch)) {
        let before = self.patch.clone();
        f(&mut self.patch);
        self.commit(before, label);
    }

    fn set_toast(&mut self, s: String) {
        self.toast = Some((s, self.now));
    }

    fn undo(&mut self) {
        self.cancel_gesture();
        if let Some(l) = self.hist.undo(&mut self.patch) {
            self.set_toast(format!("Undid: {l} · Redo"));
        }
    }

    fn redo(&mut self) {
        self.cancel_gesture();
        if let Some(l) = self.hist.redo(&mut self.patch) {
            self.set_toast(format!("Redid: {l}"));
        }
    }

    fn cancel_gesture(&mut self) {
        let g = std::mem::replace(&mut self.gesture, Gesture::None);
        match g {
            Gesture::Base { before, .. } | Gesture::Depth { before, .. } | Gesture::Wire { before, .. } => {
                self.patch = before;
                self.set_toast("Cancelled".into());
            }
            _ => {}
        }
        self.press = None;
    }

    fn ctl_name(&self, c: CtlRef) -> String {
        format!("{} {}", self.patch.module_label(c.m), self.patch.ctl_def(c).label)
    }

    fn value_str(&self, c: CtlRef) -> String {
        model::fmt_value(&self.patch.ctl_def(c).spec, self.patch.modules[c.m].values[c.c])
    }

    /// `swings 0.45 – 142 ms` for a knob with active routes, in destination units.
    fn swing_str(&self, c: CtlRef) -> Option<String> {
        let spec = self.patch.ctl_def(c).spec;
        if !matches!(spec, Spec::Knob { .. }) || self.patch.routes_to(c).all(|(_, r)| r.bypass) {
            return None;
        }
        let base = self.patch.modules[c.m].values[c.c];
        let (lo, hi) = self.patch.mod_span(c, false);
        let clamp = if base + lo < 0.0 || base + hi > 1.0 { " (clamped)" } else { "" };
        Some(format!(
            "swings {} – {}{clamp}",
            model::fmt_value(&spec, (base + lo).clamp(0.0, 1.0)),
            model::fmt_value(&spec, (base + hi).clamp(0.0, 1.0))
        ))
    }

    fn toggle_expand(&mut self, m: usize) {
        if self.geo.choose == Some(m) {
            self.geo.choose = None;
        }
        let e = !self.geo.expanded[m];
        self.geo.expanded[m] = e;
        if e {
            // Flash modulated controls that were hidden, so a route is never silently hidden.
            if let Some(r) = self.patch.routes.iter().find(|r| r.dst.m == m && !self.patch.modules[m].primary[r.dst.c]) {
                self.flash = Some((FlashTarget::Ctl(r.dst), self.now));
            }
        }
    }

    fn set_primary(&mut self, c: CtlRef, on: bool) {
        let label = format!("{} {}", if on { "Pin" } else { "Unpin" }, self.ctl_name(c));
        self.edit(label, |p| p.modules[c.m].primary[c.c] = on);
    }

    fn load_patch(&mut self, choice: PatchChoice) {
        let p = match choice {
            PatchChoice::Reference => model::reference_patch(),
            PatchChoice::Crowded => model::crowded_patch(),
        };
        self.patch_choice = choice;
        let label = match choice {
            PatchChoice::Reference => "Load reference patch",
            PatchChoice::Crowded => "Load crowded patch",
        };
        let before = std::mem::replace(&mut self.patch, p);
        self.commit(before, label.into());
        self.geo.expanded = vec![false; self.patch.modules.len()];
        self.geo.choose = None;
        self.selected = None;
        self.inspect = None;
        self.geo.selected_route = None;
    }

    fn fit(&mut self) {
        self.relayout();
        let b = self.layout.bounds.expand(12.0);
        let z = (self.canvas.width() / b.width()).min(self.canvas.height() / b.height()).clamp(0.4, 2.0);
        self.zoom = z;
        self.pan = -b.min.to_vec2() * z;
    }

    fn zoom_about(&mut self, factor: f32, at: Pos2) {
        let world = self.xf().inv(at);
        self.zoom = (self.zoom * factor).clamp(0.4, 2.5);
        self.pan = at - self.canvas.min - world.to_vec2() * self.zoom;
    }

    fn remove_route(&mut self, ri: usize) {
        let label = format!("Remove {}", self.patch.route_label(&self.patch.routes[ri]));
        self.edit(label, |p| {
            p.routes.remove(ri);
        });
        self.geo.selected_route = None;
    }

    fn invert(&mut self, ri: usize) {
        let label = format!("Invert {}", self.patch.route_label(&self.patch.routes[ri]));
        self.edit(label, |p| p.routes[ri].amount = -p.routes[ri].amount);
    }

    fn bypass(&mut self, ri: usize) {
        let r = self.patch.routes[ri];
        let label = format!("{} {}", if r.bypass { "Enable" } else { "Bypass" }, self.patch.route_label(&r));
        self.edit(label, |p| p.routes[ri].bypass = !p.routes[ri].bypass);
    }

    fn locate(&mut self, c: CtlRef) {
        if self.layout.mods[c.m].ctls[c.c].is_none() {
            self.geo.expanded[c.m] = true;
            self.relayout();
        }
        if let Some(g) = self.layout.mods[c.m].ctls[c.c] {
            let w = match g {
                geom::CtlGeo::Knob { c, .. } => c,
                geom::CtlGeo::Select { rect } => rect.center(),
            };
            let s = self.xf().p(w);
            if !self.canvas.shrink(60.0).contains(s) {
                self.pan += self.canvas.center() - s;
            }
        }
        self.flash = Some((FlashTarget::Ctl(c), self.now));
        self.inspect = Some(c);
        self.drawer = true;
    }

    // -------------------------------------------------------------------------- canvas input

    fn canvas_input(&mut self, ui: &mut egui::Ui, resp: &egui::Response) {
        let (pos, pressed, released, down, delta, mods, dbl, sec, scroll, esc) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.primary_down(),
                i.pointer.delta(),
                i.modifiers,
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
                i.pointer.secondary_pressed(),
                i.smooth_scroll_delta,
                i.key_pressed(Key::Escape),
            )
        });
        self.pointer = pos;
        let over = resp.contains_pointer();
        let world = pos.map(|p| self.xf().inv(p));
        self.hover = if over { world.and_then(|w| geom::hit_test(&self.regions, w)) } else { None };

        if esc && self.entry.is_none() {
            if !matches!(self.gesture, Gesture::None) {
                self.cancel_gesture();
            } else if self.geo.choose.is_some() {
                self.geo.choose = None;
            } else {
                self.inspect = None;
            }
        }
        if sec {
            if !matches!(self.gesture, Gesture::None) {
                self.cancel_gesture();
                return;
            }
            self.ctx_target = self.hover;
        }
        if over && scroll != Vec2::ZERO {
            if mods.command {
                self.zoom_about((scroll.y / 400.0).exp(), pos.unwrap_or(self.canvas.center()));
            } else {
                self.pan += scroll;
            }
        }
        if pressed && over && self.entry.is_none() {
            self.press = Some(Press { hit: self.hover, origin: pos.unwrap_or_default(), started: false });
        }
        let fine = if mods.shift { 0.1 } else { 1.0 };
        let mut delta = delta;
        // Start a drag gesture once the pointer has moved.
        if let (Some(pr), true, Some(p)) = (self.press.as_mut(), down, pos) {
            if !pr.started && pr.origin.distance(p) > 3.0 {
                pr.started = true;
                // The movement before the threshold counts too.
                delta = p - pr.origin;
                let hit = pr.hit;
                let before = self.patch.clone();
                self.gesture = match hit {
                    Some(Hit::Knob(c) | Hit::Pill(c)) => Gesture::Base { ctl: c, before },
                    Some(Hit::Ring(c)) => match geom::depth_route(&self.patch, c, self.geo.selected_route) {
                        Some(route) => Gesture::Depth { route, before },
                        None => Gesture::Base { ctl: c, before },
                    },
                    Some(Hit::Handle(route)) => Gesture::Depth { route, before },
                    Some(Hit::Jack(j)) => {
                        let jd = self.patch.jack_def(j);
                        if !jd.out {
                            if let Some(ci) = self.patch.has_input_cable(j) {
                                // Pick up the patched cable end (rack behaviour).
                                let from = self.patch.cables.remove(ci).from;
                                Gesture::Wire { from, before, moved_route: None, unplugged: true }
                            } else {
                                Gesture::Wire { from: j, before, moved_route: None, unplugged: false }
                            }
                        } else {
                            Gesture::Wire { from: j, before, moved_route: None, unplugged: false }
                        }
                    }
                    Some(Hit::Plug(ri)) => {
                        let r = self.patch.routes.remove(ri);
                        self.geo.selected_route = None;
                        Gesture::Wire { from: r.src, before, moved_route: Some(r), unplugged: true }
                    }
                    Some(Hit::Seg(..) | Hit::Toggle(_) | Hit::Done(_) | Hit::Pin(_)) => Gesture::None,
                    _ => Gesture::Pan,
                };
                if let Gesture::Depth { route, .. } = self.gesture {
                    self.geo.selected_route = Some(route);
                }
            }
        }
        match &mut self.gesture {
            Gesture::Base { ctl, .. } => {
                let c = *ctl;
                let v = &mut self.patch.modules[c.m].values[c.c];
                *v = (*v - delta.y / 200.0 * fine).clamp(0.0, 1.0);
            }
            Gesture::Depth { route, .. } => {
                let r = &mut self.patch.routes[*route];
                r.amount = (r.amount - delta.y / 200.0 * fine).clamp(-1.0, 1.0);
            }
            Gesture::Pan => self.pan += delta,
            _ => {}
        }
        if released {
            let g = std::mem::replace(&mut self.gesture, Gesture::None);
            let press = self.press.take();
            match g {
                Gesture::Base { ctl, before } => {
                    let label = format!("Set {} {}", self.ctl_name(ctl), self.value_str(ctl));
                    self.commit(before, label);
                }
                Gesture::Depth { route, before } => {
                    let r = self.patch.routes[route];
                    let label = format!("Depth {} {:+.0} %", self.patch.route_label(&r), r.amount * 100.0);
                    self.commit(before, label);
                }
                Gesture::Wire { from, before, moved_route, unplugged } => {
                    self.drop_wire(from, before, moved_route, unplugged, mods.alt);
                }
                Gesture::Pan => {}
                Gesture::None => {
                    if let Some(pr) = press.filter(|p| !p.started) {
                        self.click(pr.hit);
                    }
                }
            }
        }
        if dbl && over {
            let target = match self.hover {
                Some(Hit::Knob(c) | Hit::Pill(c)) => Some(EntryTarget::Base(c)),
                Some(Hit::Ring(c)) => geom::depth_route(&self.patch, c, self.geo.selected_route).map(EntryTarget::Amount),
                Some(Hit::Plug(r) | Hit::Handle(r)) => Some(EntryTarget::Amount(r)),
                _ => None,
            };
            if let Some(t) = target {
                self.open_entry(t, pos.unwrap_or_default());
            }
        }
    }

    fn open_entry(&mut self, target: EntryTarget, at: Pos2) {
        let text = match target {
            EntryTarget::Base(c) => self.value_str(c),
            EntryTarget::Amount(r) => format!("{:+.0} %", self.patch.routes[r].amount * 100.0),
        };
        self.entry = Some(Entry { target, text, pos: at + vec2(12.0, 12.0), focused: false, error: false });
    }

    fn drop_wire(&mut self, from: JackRef, before: Patch, moved: Option<Route>, unplugged: bool, alt: bool) {
        let from_out = self.patch.jack_def(from).out;
        let label = match self.hover {
            Some(Hit::Knob(c) | Hit::Ring(c) | Hit::Pill(c)) if from_out && c.m != usize::MAX => {
                let amount = moved.map(|r| r.amount).unwrap_or(if alt { 0.0 } else { model::DEFAULT_DROP_AMOUNT });
                let r = Route { src: from, dst: c, amount, bypass: moved.is_some_and(|r| r.bypass) };
                self.patch.routes.push(r);
                self.geo.selected_route = Some(self.patch.routes.len() - 1);
                self.selected = Some(from.m);
                Some(format!("Modulate {} ({:+.0} %)", self.patch.route_label(&r), amount * 100.0))
            }
            Some(Hit::Jack(j)) if j.m != from.m && self.patch.jack_def(j).out != from_out => {
                let (out, inp) = if from_out { (from, j) } else { (j, from) };
                let replaced = self.patch.has_input_cable(inp);
                if let Some(ci) = replaced {
                    self.patch.cables.remove(ci);
                }
                let c = model::Cable { from: out, to: inp };
                self.patch.cables.push(c);
                Some(format!("{} {}", if replaced.is_some() { "Replace" } else { "Connect" }, self.patch.cable_label(&c)))
            }
            _ if moved.is_some() => Some(format!("Remove {}", before.route_label(&moved.unwrap()))),
            _ if unplugged => Some("Unplug cable".to_string()),
            _ => None,
        };
        match label {
            Some(l) => {
                self.set_toast(format!("{l} · Ctrl+Z undoes"));
                self.commit(before, l);
            }
            None => self.patch = before,
        }
    }

    fn click(&mut self, hit: Option<Hit>) {
        match hit {
            Some(Hit::Toggle(m)) => self.toggle_expand(m),
            Some(Hit::Done(_)) => self.geo.choose = None,
            Some(Hit::Pin(c)) => {
                let on = !self.patch.modules[c.m].primary[c.c];
                self.set_primary(c, on);
            }
            Some(Hit::Plug(r)) => {
                self.geo.selected_route = Some(r);
                self.selected = Some(self.patch.routes[r].src.m);
                self.inspect = Some(self.patch.routes[r].dst);
                self.drawer = true;
            }
            Some(Hit::Handle(r)) => {
                self.inspect = Some(self.patch.routes[r].dst);
                self.drawer = true;
            }
            Some(Hit::Knob(c) | Hit::Pill(c) | Hit::Ring(c)) => {
                self.selected = Some(c.m);
                if self.patch.routes_to(c).next().is_some() {
                    self.inspect = Some(c);
                    self.drawer = true;
                } else {
                    self.inspect = None;
                }
            }
            Some(Hit::Seg(c, s)) => {
                self.selected = Some(c.m);
                if self.patch.modules[c.m].values[c.c] != s as f32 {
                    let Spec::Select { options, .. } = self.patch.ctl_def(c).spec else { return };
                    let label = format!("Set {} {}", self.ctl_name(c), options[s]);
                    self.edit(label, |p| p.modules[c.m].values[c.c] = s as f32);
                }
            }
            Some(Hit::Jack(j)) => {
                self.selected = Some(j.m);
                self.inspect = None;
                if let Some(ri) = self.patch.routes.iter().position(|r| r.src == j) {
                    self.geo.selected_route = Some(ri);
                    self.drawer = true;
                }
            }
            Some(Hit::Header(m) | Hit::Panel(m)) => {
                self.selected = Some(m);
                self.inspect = None;
            }
            None => {
                self.selected = None;
                self.inspect = None;
                self.geo.selected_route = None;
            }
        }
    }

    // -------------------------------------------------------------------------- panels

    fn toolbar(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>) {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("kabl").strong().size(18.0));
            ui.label(egui::RichText::new("rev-2 prototype").size(11.0).weak());
            ui.separator();
            if tb(ui, rects, "undo", "Undo", false, self.hist.can_undo()) {
                self.undo();
            }
            if tb(ui, rects, "redo", "Redo", false, self.hist.can_redo()) {
                self.redo();
            }
            ui.separator();
            ui.label("Cables");
            for (k, l, c) in [("cables.all", "All", Cables::All), ("cables.focus", "Focus", Cables::Focus), ("cables.hidden", "Hidden", Cables::Hidden)] {
                if tb(ui, rects, k, l, self.geo.cables == c, true) {
                    self.geo.cables = c;
                }
            }
            ui.separator();
            ui.label("Zoom");
            if tb(ui, rects, "zoom.out", "−", false, true) {
                let c = self.canvas.center();
                self.zoom_about(1.0 / 1.2, c);
            }
            if tb(ui, rects, "zoom.100", &format!("{:.0}%", self.zoom * 100.0), false, true) {
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
            }
            if tb(ui, rects, "zoom.in", "+", false, true) {
                let c = self.canvas.center();
                self.zoom_about(1.2, c);
            }
            if tb(ui, rects, "fit", "Fit", false, true) {
                self.fit();
            }
            ui.separator();
            if tb(ui, rects, "theme.light", "A-light", !self.dark, true) {
                self.dark = false;
            }
            if tb(ui, rects, "theme.dark", "A-dark", self.dark, true) {
                self.dark = true;
            }
            ui.separator();
            if tb(ui, rects, "patch.reference", "Reference", self.patch_choice == PatchChoice::Reference, true) {
                self.load_patch(PatchChoice::Reference);
            }
            if tb(ui, rects, "patch.crowded", "Crowded", self.patch_choice == PatchChoice::Crowded, true) {
                self.load_patch(PatchChoice::Crowded);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tb(ui, rects, "routing", "Routing", self.drawer, true) {
                    self.drawer = !self.drawer;
                }
            });
        });
    }

    fn compare_bar(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>) {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("PROTOTYPE").strong().color(Color32::WHITE).background_color(draw::hex("#c2412d")));
            ui.label(egui::RichText::new("Compare:").strong());
            ui.label("Hidden layout");
            if tb(ui, rects, "layout.rack", "Stable rack", self.geo.hidden_layout == HiddenLayout::StableRack, true) {
                self.geo.hidden_layout = HiddenLayout::StableRack;
            }
            if tb(ui, rects, "layout.compact", "Compact synth", self.geo.hidden_layout == HiddenLayout::CompactSynth, true) {
                self.geo.hidden_layout = HiddenLayout::CompactSynth;
            }
            ui.separator();
            ui.label("Expand");
            if tb(ui, rects, "exp.push", "Push", self.geo.expansion == Expansion::Push, true) {
                self.geo.expansion = Expansion::Push;
            }
            if tb(ui, rects, "exp.float", "Float", self.geo.expansion == Expansion::Float, true) {
                self.geo.expansion = Expansion::Float;
            }
            if tb(ui, rects, "autofocus", "auto-Focus", self.auto_focus, true) {
                self.auto_focus = !self.auto_focus;
            }
            ui.separator();
            ui.label("Depth");
            for (k, l, d) in [("depth.ring", "Ring", DepthGesture::RingBand), ("depth.handle", "Handle", DepthGesture::PeakHandle), ("depth.inspector", "Inspector", DepthGesture::InspectorOnly)] {
                if tb(ui, rects, k, l, self.geo.depth == d, true) {
                    self.geo.depth = d;
                }
            }
            ui.separator();
            ui.label("Skin");
            if tb(ui, rects, "skin.plates", "Plates", !self.labels_on_art, true) {
                self.labels_on_art = false;
            }
            if tb(ui, rects, "skin.art", "labels_on_art", self.labels_on_art, true) {
                self.labels_on_art = true;
            }
            ui.separator();
            if tb(ui, rects, "ab", "Env timing A/B", self.ab_open, true) {
                self.ab_open = !self.ab_open;
            }
            if tb(ui, rects, "hits", "Hit regions", self.show_hits, true) {
                self.show_hits = !self.show_hits;
            }
        });
    }

    fn drawer_ui(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>) {
        ui.horizontal(|ui| {
            ui.heading("Routing");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tb(ui, rects, "drawer.close", "Close", false, true) {
                    self.drawer = false;
                }
            });
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(c) = self.inspect.filter(|c| c.m < self.patch.modules.len()) {
                self.inspector(ui, rects, c);
                ui.add_space(8.0);
            }
            let src = self
                .geo
                .selected_route
                .and_then(|r| self.patch.routes.get(r).map(|r| r.src))
                .or_else(|| self.selected.and_then(|m| self.patch.routes.iter().find(|r| r.src.m == m).map(|r| r.src)));
            if let Some(src) = src.filter(|_| self.inspect.is_none()) {
                let routes: Vec<usize> = (0..self.patch.routes.len()).filter(|&i| self.patch.routes[i].src == src).collect();
                ui.label(egui::RichText::new(format!("SELECTED SOURCE · {} {} · {} routes", self.patch.module_label(src.m), self.patch.jack_def(src).label.to_lowercase(), routes.len())).small().strong());
                for ri in routes {
                    self.route_card(ui, rects, ri);
                }
                ui.add_space(8.0);
            }
            ui.label(egui::RichText::new(format!("ALL CONNECTIONS · {}", self.patch.cables.len() + self.patch.routes.len())).small().strong());
            let th = theme(self.dark);
            for c in self.patch.cables.clone() {
                let col = th.sig(self.patch.jack_def(c.from).sig);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("▌").color(col));
                    ui.label(self.patch.cable_label(&c));
                });
            }
            for ri in 0..self.patch.routes.len() {
                let r = self.patch.routes[ri];
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("▌").color(if r.bypass { Color32::GRAY } else { th.cv }));
                    let sel = self.geo.selected_route == Some(ri);
                    let resp = ui.selectable_label(sel, format!("{}  {:+.0} %{}", self.patch.route_label(&r), r.amount * 100.0, if r.bypass { " · bypassed" } else { "" }));
                    rects.insert(format!("row.{}", script::route_key(&self.patch, &r)), resp.rect);
                    if resp.clicked() {
                        self.geo.selected_route = Some(ri);
                        self.selected = Some(r.src.m);
                        self.inspect = None;
                    }
                });
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new("+ NEW ROUTE (works in every cable mode)").small().strong());
            self.new_route_ui(ui, rects);
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>, c: CtlRef) {
        let th = theme(self.dark);
        ui.label(egui::RichText::new(format!("DESTINATION · {}", self.ctl_name(c))).small().strong());
        ui.horizontal(|ui| {
            ui.label(format!("Base {}", self.value_str(c)));
            if tb(ui, rects, "inspector.reset", "Reset", false, true) {
                let d = self.patch.ctl_def(c);
                let t = match d.spec {
                    Spec::Knob { default, .. } => model::to_t(&d.spec, default),
                    Spec::Select { default, .. } => default as f32,
                };
                let label = format!("Reset {}", self.ctl_name(c));
                self.edit(label, |p| p.modules[c.m].values[c.c] = t);
            }
            if tb(ui, rects, "inspector.type", "Type…", false, true) {
                let at = ui.min_rect().left_bottom();
                self.open_entry(EntryTarget::Base(c), at);
            }
        });
        if let Some(s) = self.swing_str(c) {
            ui.label(egui::RichText::new(s).small());
        }
        // Range bar: ▲ base, thin bracket per source, thick bar = result.
        let routes: Vec<usize> = self.patch.routes_to(c).map(|(i, _)| i).collect();
        let h = 30.0 + 8.0 * routes.len() as f32;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let p = ui.painter();
        let x = |t: f32| rect.left() + 6.0 + (rect.width() - 12.0) * t.clamp(0.0, 1.0);
        p.line_segment([pos2(x(0.0), rect.top() + 12.0), pos2(x(1.0), rect.top() + 12.0)], egui::Stroke::new(2.0, Color32::from_gray(90)));
        let base = self.patch.modules[c.m].values[c.c];
        let (lo, hi) = self.patch.mod_span(c, false);
        p.line_segment([pos2(x(base + lo), rect.top() + 12.0), pos2(x(base + hi), rect.top() + 12.0)], egui::Stroke::new(7.0, th.cv));
        p.add(egui::Shape::convex_polygon(vec![pos2(x(base), rect.top() + 17.0), pos2(x(base) - 6.0, rect.top() + 27.0), pos2(x(base) + 6.0, rect.top() + 27.0)], Color32::WHITE, egui::Stroke::NONE));
        for (k, &ri) in routes.iter().enumerate() {
            let (a, b) = self.patch.route_span(&self.patch.routes[ri]);
            let y = rect.top() + 32.0 + 8.0 * k as f32;
            let col = if self.patch.routes[ri].bypass { Color32::GRAY } else { th.cv.lerp_to_gamma(Color32::WHITE, 0.25 * k as f32) };
            p.line_segment([pos2(x(base + a), y), pos2(x(base + b), y)], egui::Stroke::new(2.0, col));
            for e in [a, b] {
                p.line_segment([pos2(x(base + e), y - 3.0), pos2(x(base + e), y + 3.0)], egui::Stroke::new(2.0, col));
            }
        }
        ui.label(egui::RichText::new(format!("{} source{}", routes.len(), if routes.len() == 1 { "" } else { "s" })).small());
        for ri in routes {
            self.route_card(ui, rects, ri);
        }
    }

    fn route_card(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>, ri: usize) {
        if ri >= self.patch.routes.len() {
            return;
        }
        let r = self.patch.routes[ri];
        let key = script::route_key(&self.patch, &r);
        let sel = self.geo.selected_route == Some(ri);
        let th = theme(self.dark);
        egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { th.cv } else { Color32::from_gray(80) }))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new(self.patch.route_label(&r)).strong());
                let dst_def = self.patch.ctl_def(r.dst);
                let rate = if self.patch.modules[r.dst.m].kind == Kind::Adsr || matches!(dst_def.spec, Spec::Select { .. }) || self.patch.modules[r.dst.m].kind == Kind::Lfo { " · block rate" } else { "" };
                let feedback = if r.src.m == r.dst.m { " · 1-block delay (feedback)" } else { "" };
                let pol = if self.patch.source_bipolar(r.src) { "bipolar source" } else { "unipolar source" };
                ui.label(egui::RichText::new(format!("{pol} · per voice{rate}{feedback}")).small().weak());
                ui.spacing_mut().slider_width = 170.0;
                let mut pct = r.amount * 100.0;
                let resp = ui.add(egui::Slider::new(&mut pct, -100.0..=100.0).suffix(" %").text("Amount").fixed_decimals(0));
                rects.insert(format!("card.amount.{key}"), resp.rect);
                if resp.changed() {
                    if self.slider_before.as_ref().is_none_or(|(i, _)| *i != ri) {
                        self.slider_before = Some((ri, self.patch.clone()));
                    }
                    self.patch.routes[ri].amount = (pct / 100.0).clamp(-1.0, 1.0);
                }
                if !resp.dragged() && !resp.has_focus() {
                    if let Some((i, before)) = self.slider_before.take() {
                        if i == ri {
                            let label = format!("Depth {} {:+.0} %", self.patch.route_label(&self.patch.routes[ri]), self.patch.routes[ri].amount * 100.0);
                            self.commit(before, label);
                        } else {
                            self.slider_before = Some((i, before));
                        }
                    }
                }
                let r = self.patch.routes[ri];
                let base = self.patch.modules[r.dst.m].values[r.dst.c];
                if let Spec::Knob { .. } = dst_def.spec {
                    let (a, b) = self.patch.route_span(&r);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} · swings {} – {}",
                            dst_def.label,
                            self.value_str(r.dst),
                            model::fmt_value(&dst_def.spec, (base + a).clamp(0.0, 1.0)),
                            model::fmt_value(&dst_def.spec, (base + b).clamp(0.0, 1.0))
                        ))
                        .small(),
                    );
                }
                if r.bypass {
                    ui.label(egui::RichText::new("Bypassed: no effect, amount kept").small().color(Color32::GRAY));
                }
                ui.horizontal(|ui| {
                    if tb(ui, rects, &format!("card.invert.{key}"), "Invert", false, true) {
                        self.invert(ri);
                    }
                    if tb(ui, rects, &format!("card.bypass.{key}"), "Bypass", r.bypass, true) {
                        self.bypass(ri);
                    }
                    if tb(ui, rects, &format!("card.remove.{key}"), "Remove", false, true) {
                        self.remove_route(ri);
                    }
                    if tb(ui, rects, &format!("card.locate.{key}"), "Locate", false, true) {
                        self.locate(r.dst);
                    }
                });
            });
    }

    fn new_route_ui(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>) {
        let srcs: Vec<JackRef> = (0..self.patch.modules.len())
            .flat_map(|m| (0..self.patch.modules[m].def().jacks.len()).map(move |j| JackRef { m, j }))
            .filter(|j| self.patch.jack_def(*j).out)
            .collect();
        let dsts: Vec<CtlRef> = (0..self.patch.modules.len())
            .flat_map(|m| (0..self.patch.modules[m].def().controls.len()).map(move |c| CtlRef { m, c }))
            .collect();
        let name_src = |p: &Patch, j: JackRef| format!("{} {}", p.module_label(j.m), p.jack_def(j).label.to_lowercase());
        let name_dst = |p: &Patch, c: CtlRef| {
            format!("{} {}{}", p.module_label(c.m), p.ctl_def(c).label, if p.modules[c.m].primary[c.c] { "" } else { " (hidden)" })
        };
        let cur_s = self.new_route.0.map(|j| name_src(&self.patch, j)).unwrap_or("source…".into());
        let cur_d = self.new_route.1.map(|c| name_dst(&self.patch, c)).unwrap_or("destination…".into());
        egui::ComboBox::from_id_salt("nr_src").selected_text(cur_s).show_ui(ui, |ui| {
            for j in &srcs {
                ui.selectable_value(&mut self.new_route.0, Some(*j), name_src(&self.patch, *j));
            }
        });
        egui::ComboBox::from_id_salt("nr_dst").selected_text(cur_d).show_ui(ui, |ui| {
            for c in &dsts {
                ui.selectable_value(&mut self.new_route.1, Some(*c), name_dst(&self.patch, *c));
            }
        });
        let ok = matches!(self.new_route, (Some(_), Some(_)));
        if tb(ui, rects, "newroute.add", "Add route (+25 %)", false, ok) {
            let (Some(src), Some(dst)) = self.new_route else { return };
            let r = Route { src, dst, amount: model::DEFAULT_DROP_AMOUNT, bypass: false };
            let label = format!("Modulate {} (+25 %)", self.patch.route_label(&r));
            self.edit(label, |p| p.routes.push(r));
            self.geo.selected_route = Some(self.patch.routes.len() - 1);
        }
    }

    fn entry_ui(&mut self, ctx: &egui::Context) {
        let Some(e) = self.entry.as_mut() else { return };
        let mut close = false;
        let mut commit = false;
        egui::Area::new(egui::Id::new("entry")).fixed_pos(e.pos).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let hint = match e.target {
                    EntryTarget::Base(_) => "value, e.g. 12 ms · 1.2k · 2 s",
                    EntryTarget::Amount(_) => "amount, e.g. +40 % · -25",
                };
                ui.label(egui::RichText::new(hint).small());
                let resp = ui.add(egui::TextEdit::singleline(&mut e.text).desired_width(160.0).id(egui::Id::new("entry_text")));
                if !e.focused {
                    resp.request_focus();
                    e.focused = true;
                }
                if e.error {
                    ui.label(egui::RichText::new("not understood").small().color(Color32::from_rgb(255, 120, 100)));
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                } else if ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit = true;
                }
            });
        });
        if commit {
            let e = self.entry.as_ref().unwrap();
            let (target, text) = (e.target, e.text.clone());
            let ok = match target {
                EntryTarget::Base(c) => model::parse_value(&self.patch.ctl_def(c).spec, &text).map(|t| {
                    let label = format!("Set {} {}", self.ctl_name(c), model::fmt_value(&self.patch.ctl_def(c).spec, t));
                    self.edit(label, |p| p.modules[c.m].values[c.c] = t);
                }),
                EntryTarget::Amount(r) => model::parse_amount(&text).map(|a| {
                    let label = format!("Depth {} {:+.0} %", self.patch.route_label(&self.patch.routes[r]), a * 100.0);
                    self.edit(label, |p| p.routes[r].amount = a);
                }),
            };
            match ok {
                Some(()) => close = true,
                None => self.entry.as_mut().unwrap().error = true,
            }
        }
        if close {
            self.entry = None;
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>) {
        let Some(hit) = self.ctx_target else {
            ui.label("Rack");
            return;
        };
        match hit {
            Hit::Knob(c) | Hit::Pill(c) | Hit::Ring(c) | Hit::Seg(c, _) | Hit::Pin(c) => {
                ui.label(egui::RichText::new(self.ctl_name(c)).strong());
                if mb(ui, rects, "menu.type", "Type value…") {
                    let at = self.pointer.unwrap_or_default();
                    self.open_entry(EntryTarget::Base(c), at);
                    ui.close();
                }
                if mb(ui, rects, "menu.inspect", "Inspect") {
                    self.inspect = Some(c);
                    self.drawer = true;
                    ui.close();
                }
                let prim = self.patch.modules[c.m].primary[c.c];
                if mb(ui, rects, "menu.pin", if prim { "Remove from face" } else { "Pin to face (primary)" }) {
                    self.set_primary(c, !prim);
                    ui.close();
                }
                if self.patch.routes_to(c).next().is_some() && mb(ui, rects, "menu.unmod", "Remove all modulation") {
                    let label = format!("Remove modulation on {}", self.ctl_name(c));
                    self.edit(label, |p| p.routes.retain(|r| r.dst != c));
                    ui.close();
                }
            }
            Hit::Plug(r) | Hit::Handle(r) => {
                ui.label(egui::RichText::new(self.patch.route_label(&self.patch.routes[r])).strong());
                if mb(ui, rects, "menu.invert", "Invert") {
                    self.invert(r);
                    ui.close();
                }
                if mb(ui, rects, "menu.bypass", "Bypass") {
                    self.bypass(r);
                    ui.close();
                }
                if mb(ui, rects, "menu.remove", "Remove") {
                    self.remove_route(r);
                    ui.close();
                }
            }
            Hit::Header(m) | Hit::Panel(m) | Hit::Toggle(m) | Hit::Done(m) => {
                ui.label(egui::RichText::new(self.patch.modules[m].def().name).strong());
                if mb(ui, rects, "menu.choose", "Choose primary controls…") {
                    self.geo.choose = Some(m);
                    ui.close();
                }
                if mb(ui, rects, "menu.resetprimary", "Reset primary to module default") {
                    let d: Vec<bool> = self.patch.modules[m].def().controls.iter().map(|c| c.primary).collect();
                    self.edit("Reset primary controls".into(), |p| p.modules[m].primary = d);
                    ui.close();
                }
                if self.patch.modules[m].hidden_count() > 0 && mb(ui, rects, "menu.expand", if self.geo.expanded[m] { "Collapse" } else { "Expand" }) {
                    self.toggle_expand(m);
                    ui.close();
                }
            }
            Hit::Jack(j) => {
                ui.label(format!("{} {}", self.patch.module_label(j.m), self.patch.jack_def(j).label));
                ui.label(egui::RichText::new("Drag to connect; drag onto a knob to modulate").small());
            }
        }
    }

    fn ab_window(&mut self, ctx: &egui::Context) {
        let mut open = self.ab_open;
        let lfo = self.patch.modules.iter().position(|m| m.kind == Kind::Lfo);
        let env = self.patch.modules.iter().position(|m| m.kind == Kind::Adsr);
        egui::Window::new("Envelope-time modulation · A/B").open(&mut open).default_width(700.0).show(ctx, |ui| {
            ui.label(egui::RichText::new("SIMULATION, not engine output. Production kabl_modules::dsp FullAdsr/FullLfo/FullOsc/Svf, unmodified; route summing and the two timing policies are this harness's. Only the policy differs between A and B.").small());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.ab_slow, false, "Current patch values");
                ui.radio_value(&mut self.ab_slow, true, "Slow pad (stress case)");
            });
            let sc = if self.ab_slow {
                envtime::Scenario::slow_pad()
            } else {
                let (Some(lfo), Some(env)) = (lfo, env) else { return };
                let v = |m: usize, id: &str| {
                    let c = self.patch.ctl(m, id);
                    model::to_value(&self.patch.ctl_def(c).spec, self.patch.modules[m].values[c.c])
                };
                let attack = self.patch.ctl(env, "attack_ms");
                let amount = self.patch.routes.iter().filter(|r| r.dst == attack && !r.bypass && r.src == self.patch.jack(lfo, "out")).map(|r| r.amount).sum::<f32>();
                envtime::Scenario::from_patch(v(env, "attack_ms"), v(lfo, "rate_hz"), v(lfo, "waveform") as usize, amount)
            };
            let key = format!("{sc:?}");
            if self.ab_cache.as_ref().is_none_or(|(k, ..)| *k != key) {
                let a = envtime::render(&sc, envtime::Policy::Continuous);
                let b = envtime::render(&sc, envtime::Policy::StageStart);
                self.ab_cache = Some((key, a, b, sc.clone()));
            }
            let (_, a, b, sc) = self.ab_cache.as_ref().unwrap();
            ui.label(format!(
                "Attack base {:.1} ms · LFO {:.2} Hz {:?} → Attack {:+.0} % · {} notes · block {} samples",
                sc.attack_ms,
                sc.lfo_hz,
                sc.lfo_wave,
                sc.amount * 100.0,
                sc.notes.len(),
                envtime::BLOCK
            ));
            if sc.amount == 0.0 {
                ui.colored_label(Color32::from_rgb(255, 180, 90), "No active LFO → Attack route in the patch: both policies are identical. Connect one, or pick the stress case.");
            }
            let th = theme(self.dark);
            ui.horizontal(|ui| {
                ui.colored_label(th.cv, "■ A: continuous (per block)");
                ui.colored_label(th.audio, "■ B: sampled at stage start");
            });
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
            draw::env_plot(ui.painter(), rect, &a.env, &b.env, sc.seconds, &th);
            egui::Grid::new("rise").striped(true).show(ui, |ui| {
                ui.label("note");
                ui.label("A rise to 0.9");
                ui.label("B rise to 0.9");
                ui.end_row();
                for (i, (x, y)) in a.rise_90.iter().zip(&b.rise_90).enumerate() {
                    let f = |v: &Option<f32>| v.map(|s| format!("{:.0} ms", s * 1000.0)).unwrap_or("never (gate ended)".into());
                    ui.label(format!("{}", i + 1));
                    ui.label(f(x));
                    ui.label(f(y));
                    ui.end_row();
                }
            });
            ui.label(format!("RMS difference: envelope {:.4} · audio {:.4}", envtime::rms_diff(&a.env, &b.env), envtime::rms_diff(&a.audio, &b.audio)));
            ui.separator();
            ui.label(egui::RichText::new("Pictures do not settle this. Listen: cargo run -p kabl-ui --example rev2_env_ab --release  →  target/rev2-proto/env-ab/*.wav").strong());
        });
        self.ab_open = open;
    }
}

/// Menu button that records its rect for the script driver.
fn mb(ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>, key: &str, label: &str) -> bool {
    let resp = ui.button(label);
    rects.insert(key.to_string(), resp.rect);
    resp.clicked()
}

/// Toolbar-style button that records its rect for the script driver.
fn tb(ui: &mut egui::Ui, rects: &mut HashMap<String, Rect>, key: &str, label: &str, on: bool, enabled: bool) -> bool {
    let resp = ui.add_enabled(enabled, egui::Button::selectable(on, label));
    rects.insert(key.to_string(), resp.rect);
    resp.clicked()
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(mut s) = self.script.take() {
            s.frame(self, ctx, raw);
            self.script = Some(s);
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        let th = theme(self.dark);
        let mut vis = egui::Visuals::dark();
        vis.panel_fill = th.chrome;
        vis.window_fill = th.chrome;
        vis.override_text_color = Some(th.ctext);
        vis.selection.bg_fill = th.sel;
        ctx.set_visuals(vis);
        let mut rects = HashMap::new();

        // Redo first: egui's logical key match lets Ctrl+Z also match Ctrl+Shift+Z.
        let (redo, undo) = ctx.input_mut(|i| {
            let redo = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z) || i.consume_key(egui::Modifiers::COMMAND, Key::Y);
            (redo, i.consume_key(egui::Modifiers::COMMAND, Key::Z))
        });
        if self.entry.is_none() {
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            }
        }

        egui::Panel::top("toolbar").exact_size(44.0).show(ui, |ui| self.toolbar(ui, &mut rects));
        egui::Panel::bottom("compare").exact_size(34.0).show(ui, |ui| self.compare_bar(ui, &mut rects));
        if self.drawer {
            egui::Panel::right("drawer").exact_size(336.0).resizable(false).show(ui, |ui| self.drawer_ui(ui, &mut rects));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(th.rack)).show(ui, |ui| {
            self.canvas = ui.max_rect();
            self.relayout();
            let resp = ui.allocate_rect(self.canvas, Sense::click_and_drag());
            self.canvas_input(ui, &resp);
            self.relayout();
            let painter = ui.painter_at(self.canvas);
            draw::canvas(self, &painter);
            self.tooltip(&painter);
            if let Some((t, at)) = &self.toast {
                if self.now - at < 3.0 {
                    let g = painter.layout_no_wrap(t.clone(), egui::FontId::proportional(13.0), Color32::WHITE);
                    let r = Rect::from_center_size(pos2(self.canvas.center().x, self.canvas.bottom() - 28.0), g.size() + vec2(24.0, 12.0));
                    painter.rect_filled(r, CornerRadius::same(6), Color32::from_black_alpha(220));
                    painter.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
                }
            }
            resp.context_menu(|ui| self.context_menu(ui, &mut rects));
        });
        self.entry_ui(&ctx);
        if self.ab_open {
            self.ab_window(&ctx);
        }
        self.ui_rects = rects;
        if self.flash.is_some() || self.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl App {
    fn tooltip(&self, p: &egui::Painter) {
        let Some(pos) = self.pointer else { return };
        let lines: Vec<String> = match (&self.gesture, self.hover) {
            (Gesture::Wire { from, .. }, Some(Hit::Knob(c) | Hit::Ring(c) | Hit::Pill(c))) if self.patch.jack_def(*from).out => {
                vec![format!("Release to modulate {} (+25 %)", self.ctl_name(c)), "Alt-release: +0 %".into()]
            }
            (Gesture::Wire { .. }, _) => vec!["Drop on a jack or a knob · Esc cancels".into()],
            (Gesture::Depth { route, .. }, _) => {
                let r = self.patch.routes[*route];
                let mut v = vec![format!("{:+.0} % · {}", r.amount * 100.0, self.patch.route_label(&r))];
                v.extend(self.swing_str(r.dst));
                v.push("Shift = fine · Esc cancels".into());
                v
            }
            (Gesture::Base { ctl, .. }, _) => {
                let mut v = vec![format!("{} {}", self.ctl_name(*ctl), self.value_str(*ctl))];
                v.extend(self.swing_str(*ctl));
                v
            }
            (Gesture::None, Some(Hit::Ring(c))) => vec![format!("Drag: depth of {}", self.ctl_name(c)), "Click: inspect".into()],
            (Gesture::None, Some(Hit::Handle(r))) => vec![format!("Drag: depth {:+.0} %", self.patch.routes[r].amount * 100.0)],
            (Gesture::None, Some(Hit::Plug(r))) => {
                let rt = self.patch.routes[r];
                vec![format!("{} {:+.0} %{}", self.patch.route_label(&rt), rt.amount * 100.0, if rt.bypass { " · bypassed" } else { "" })]
            }
            (Gesture::None, Some(Hit::Toggle(m))) => {
                let hidden: Vec<String> = self
                    .patch
                    .routes
                    .iter()
                    .filter(|r| r.dst.m == m && self.layout.mods[m].ctls[r.dst.c].is_none())
                    .map(|r| format!("◆ hidden: {}", self.patch.route_label(r)))
                    .collect();
                if hidden.is_empty() {
                    return;
                }
                hidden
            }
            (Gesture::None, Some(Hit::Knob(c) | Hit::Pill(c))) => match self.swing_str(c) {
                Some(s) => vec![format!("{} {} · {s}", self.ctl_name(c), self.value_str(c))],
                None => return,
            },
            _ => return,
        };
        let text = lines.join("\n");
        let g = p.layout_no_wrap(text, egui::FontId::proportional(12.5), Color32::WHITE);
        let r = Rect::from_min_size(pos + vec2(16.0, 18.0), g.size() + vec2(14.0, 10.0));
        let r = r.translate(vec2((self.canvas.right() - r.right()).min(0.0), (self.canvas.bottom() - r.bottom()).min(0.0)));
        p.rect_filled(r, CornerRadius::same(5), Color32::from_rgba_unmultiplied(20, 20, 22, 235));
        p.galley(r.min + vec2(7.0, 5.0), g, Color32::WHITE);
    }
}

/// The mockups use IBM Plex; egui's bundled fonts lack ← → ◆ ⚠. Use system fonts when present,
/// else fall back to egui's defaults (arrows then render as boxes).
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = "/usr/share/fonts/truetype";
    let load = [
        ("plex-sans", format!("{dir}/ibm-plex/IBMPlexSans-Regular.ttf"), egui::FontFamily::Proportional, 0),
        ("plex-mono", format!("{dir}/ibm-plex/IBMPlexMono-Regular.ttf"), egui::FontFamily::Monospace, 0),
        ("dejavu", format!("{dir}/dejavu/DejaVuSans.ttf"), egui::FontFamily::Proportional, 1),
        ("dejavu", format!("{dir}/dejavu/DejaVuSans.ttf"), egui::FontFamily::Monospace, 1),
    ];
    for (name, path, family, at) in load {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        fonts.font_data.entry(name.to_string()).or_insert_with(|| std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        let list = fonts.families.entry(family).or_default();
        list.insert(at.min(list.len()), name.to_string());
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let (w, h) = arg("--size")
        .and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().ok(), b.parse().ok())))
        .and_then(|(a, b)| Some((a?, b?)))
        .unwrap_or((1440.0, 900.0));
    let dark = args.iter().any(|a| a == "--dark");
    let script = match arg("--script") {
        Some(path) => Some(script::Script::load(&path, arg("--shots").unwrap_or("target/rev2-proto/shots".into())).expect("readable script")),
        None => None,
    };
    let ppp: Option<f32> = arg("--ppp").and_then(|s| s.parse().ok());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([w, h]).with_min_inner_size([1024.0, 700.0]).with_title("kabl · revision-2 prototype"),
        ..Default::default()
    };
    eframe::run_native(
        "kabl-rev2-proto",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            if let Some(p) = ppp {
                cc.egui_ctx.set_pixels_per_point(p);
            }
            Ok(Box::new(App::new(dark, script)))
        }),
    )
}
