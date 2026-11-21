//! The first-run tour (UI spec 7): four steps over the start patch, each skippable with Esc,
//! never shown again once dismissed (`Prefs::first_run_done`).
//!
//! 1. Sounds is open; a toast says to press play on any sound.
//! 2. A sound is loaded: the view goes to the rack, a stopped sequence runs, a toast says the
//!    sound flows along the glowing cables.
//! 3. The rack dims and the filter's cutoff is spotlit, with a card: "Turn this knob".
//! 4. "Make it yours": a toast about unplugging a cable and browsing more sounds.
//!
//! Steps 1, 2 and 4 are toasts (severity `Info`, which stays until the tour takes it away or it
//! is dismissed); step 3 is the spotlight. The patch keeps playing under the scrim.

use crate::first_run::find_filter;
use crate::kit::{self, Tone};
use crate::style::{alpha, Role, Style};
use crate::{PatchEditor, UiState};
use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke};
use kabl_core::ModuleId;

/// The four steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Sounds is open: "Press play on any sound to hear it."
    Sounds,
    /// A sound is loaded and plays: "Sound flows along the glowing cables."
    Flows,
    /// The rack dims and the filter cutoff is spotlit: "Turn this knob".
    Knob,
    /// "Make it yours."
    Own,
}

impl Step {
    /// 1 to 4.
    pub fn number(self) -> usize {
        self as usize + 1
    }
}

/// The tour's state, in `UiState::first_run` while it runs.
#[derive(Debug)]
pub struct Tour {
    pub step: Step,
    /// Egui time the step began (`None` until its first frame).
    since: Option<f64>,
    /// The cutoff when the spotlight came up: a change means the user touched it.
    knob0: Option<f32>,
    /// The step's toast has been put up. Once: a dismissed toast stays dismissed.
    shown: bool,
}

/// Starts the tour unless it was dismissed before.
pub fn begin(done: bool) -> Option<Tour> {
    (!done).then(|| begin_at(Step::Sounds))
}

/// A tour at `step` (captures and tests).
pub fn begin_at(step: Step) -> Tour {
    Tour {
        step,
        since: None,
        knob0: None,
        shown: false,
    }
}

/// The toasts of steps 1, 2 and 4 (also how they are told apart from other toasts).
pub const TOASTS: [&str; 3] = [
    "Press play on any sound to hear it.",
    "Sound flows along the glowing cables.",
    "Make it yours",
];
const OWN_DETAIL: &str = "Drag a cable end out of its jack to unplug it, or browse more sounds.";
const OWN_ACTION: &str = "Browse sounds";

/// The mockup's spotlight circle, card and scrim (`#08090A` at 0.66).
const SPOT_R: f32 = 58.0;
const CARD: egui::Vec2 = vec2(284.0, 168.0);
const SCRIM: Color32 = Color32::from_rgba_premultiplied(5, 6, 7, 168);
const PULSE_SECS: f64 = 1.8;
/// How long step 2's toast stays before the spotlight, and step 4's before the tour ends.
const FLOW_SECS: f64 = 4.0;
const OWN_SECS: f64 = 20.0;
/// How long to wait for the cutoff knob to be on screen before going on without the spotlight.
const KNOB_WAIT_SECS: f64 = 3.0;

fn put_toast(
    ui_state: &mut UiState,
    now: f64,
    title: &str,
    detail: Option<&str>,
    actions: &[&str],
) {
    ui_state.toasts.push(kit::g4::Toast {
        sev: kit::g4::Sev::Info,
        title: title.into(),
        detail: detail.map(Into::into),
        actions: actions.iter().map(|a| (*a).into()).collect(),
        expanded: true,
        born: now,
        leaving: None,
    });
}

fn take_toasts(ui_state: &mut UiState) {
    ui_state
        .toasts
        .retain(|t| !TOASTS.contains(&t.title.as_str()));
}

fn go(tour: &mut Tour, step: Step, now: f64) {
    tour.step = step;
    tour.since = Some(now);
    tour.knob0 = None;
    tour.shown = false;
}

/// Ends the tour for good: its toasts go and it is never shown again.
pub fn finish(ui_state: &mut UiState) {
    take_toasts(ui_state);
    ui_state.first_run = None;
    if let Some(lib) = ui_state.library.as_mut() {
        lib.prefs.first_run_done = true;
        let _ = lib.save_prefs();
    }
}

/// The filter cutoff's position 0..1, as stored.
fn cutoff_of(editor: &PatchEditor, id: ModuleId) -> Option<f32> {
    let info = kabl_modules::registry::info_for(&editor.state().modules.get(&id)?.kind)?;
    let p = info.params.iter().find(|p| p.name == "cutoff_hz")?;
    Some(p.to_norm(crate::routing::base_value(editor.state(), id, p)))
}

/// One frame of the tour: advances it, keeps its toasts, draws the spotlight. Called before the
/// toasts are drawn. `area` is the rack's page (the top bar and the status bar stay as they
/// are). Does nothing while `UiState::first_run` is `None`.
pub fn frame(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    area: Rect,
) {
    let Some(mut tour) = ui_state.first_run.take() else {
        return;
    };
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let since = *tour.since.get_or_insert(now);

    // Esc skips the tour, unless Esc belongs to something else (a dialog, a menu, a text field).
    let free = ui_state.browser.dialog.is_none()
        && !ctx.egui_wants_keyboard_input()
        && !egui::Popup::is_any_open(&ctx);
    if free && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        finish(ui_state);
        return;
    }

    let mut done = false;
    match tour.step {
        Step::Sounds => {
            if !tour.shown {
                tour.shown = true;
                put_toast(ui_state, now, TOASTS[0], None, &[]);
            }
            // A sound from the library or a folder is loaded: on to step 2.
            let loaded = ui_state
                .doc
                .as_ref()
                .is_some_and(|d| !matches!(d.origin, crate::browser::DocOrigin::New));
            if loaded {
                take_toasts(ui_state);
                // The view goes to the rack, and a sequence that loaded stopped runs.
                ui_state.browser_open = false;
                let clocks: Vec<_> = editor
                    .state()
                    .modules
                    .iter()
                    .filter(|(_, m)| m.kind == "clock")
                    .map(|(&id, _)| (id, kabl_modules::builtins::Transport::Run))
                    .collect();
                ui_state.transport.extend(clocks);
                go(&mut tour, Step::Flows, now);
            }
        }
        Step::Flows => {
            if !tour.shown {
                tour.shown = true;
                put_toast(ui_state, now, TOASTS[1], None, &[]);
            }
            if now - since > FLOW_SECS {
                take_toasts(ui_state);
                go(&mut tour, Step::Knob, now);
            }
        }
        Step::Knob => {
            let filter = find_filter(editor.state());
            let knob =
                filter.and_then(|id| ui_state.hits.get(&format!("knob:{id}.cutoff_hz")).copied());
            match (filter, knob) {
                (Some(id), Some(knob)) => {
                    let base = cutoff_of(editor, id);
                    let first = *tour.knob0.get_or_insert(base.unwrap_or(0.0));
                    let touched = base.is_some_and(|b| (b - first).abs() > 1e-4);
                    let choice = spotlight(ui_state, &ctx, st, knob, area, id, now);
                    if touched || matches!(choice, Some(Choice::Next)) {
                        go(&mut tour, Step::Own, now);
                    } else if matches!(choice, Some(Choice::Skip)) {
                        done = true;
                    }
                }
                // No filter, or its cutoff is not on screen (another view): wait a little, then
                // go on without the spotlight.
                _ if now - since > KNOB_WAIT_SECS => go(&mut tour, Step::Own, now),
                _ => {}
            }
        }
        Step::Own => {
            // The user unplugged the cable into Output: the rack's empty-state hint offers to
            // plug it back or to start from another sound. The tour waits while it shows.
            let hint = nothing_reaches_output(editor);
            if hint {
                unplugged_hint(editor, ui_state, &ctx, st, area);
            }
            if !tour.shown {
                tour.shown = true;
                put_toast(ui_state, now, TOASTS[2], Some(OWN_DETAIL), &[OWN_ACTION]);
            } else if !hint
                && (!ui_state.toasts.iter().any(|t| t.title == TOASTS[2]) || now - since > OWN_SECS)
            {
                // Dismissed, acted on, or left alone long enough.
                done = true;
            }
        }
    }
    if done {
        finish(ui_state);
    } else {
        ui_state.first_run = Some(tour);
    }
}

/// The patch has an output and no cable reaches it.
fn nothing_reaches_output(editor: &PatchEditor) -> bool {
    let state = editor.state();
    let outs: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "out")
        .map(|(&id, _)| id)
        .collect();
    !outs.is_empty()
        && !state
            .cables
            .values()
            .any(|c| matches!(&c.to, kabl_core::PortRef::Module { id, .. } if outs.contains(id)))
}

/// The rack's empty-state card for an unplugged output: plug the cable back (undo), or browse.
fn unplugged_hint(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ctx: &egui::Context,
    st: &Style,
    screen: Rect,
) {
    let at = pos2(screen.center().x - 260.0, screen.center().y - 150.0);
    let mut plug = false;
    let mut browse = false;
    egui::Area::new(egui::Id::new("first-run-unplugged"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_width(520.0);
            let r = kit::g4::empty_state(
                ui,
                st,
                kit::Ic::CableOff,
                "Nothing reaches the output",
                "You unplugged the cable. Plug it back, or start from another sound.",
                &[
                    kit::g4::EmptyAction {
                        label: "Plug it back",
                        primary: true,
                        icon: kit::Ic::Cable,
                    },
                    kit::g4::EmptyAction {
                        label: "Browse sounds",
                        primary: false,
                        icon: kit::Ic::Search,
                    },
                ],
                None,
            );
            ui_state.record("tour-plug-back".into(), r[0].rect);
            ui_state.record("tour-browse".into(), r[1].rect);
            plug = r[0].clicked();
            browse = r[1].clicked();
        });
    if plug {
        editor.undo();
    }
    if browse {
        ui_state.browser_open = true;
    }
}

/// What the user chose on the spotlight card.
enum Choice {
    Next,
    Skip,
}

/// The step-3 spotlight: the rack dims except a circle around `knob`, a ring pulses outward from
/// it, and a card says what to do. The scrim takes the pointer everywhere but the circle, so the
/// knob stays usable and nothing else is.
fn spotlight(
    ui_state: &mut UiState,
    ctx: &egui::Context,
    st: &Style,
    knob: Rect,
    screen: Rect,
    filter: ModuleId,
    now: f64,
) -> Option<Choice> {
    let c = knob.center();
    let r = SPOT_R.max(knob.width() * 0.5 + 14.0);
    let hole = Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r));
    egui::Area::new(egui::Id::new("first-run-scrim"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let p = ui.painter();
            paint_scrim(p, screen, c, r);
            // The accent ring, and one that pulses outward every 1.8 s.
            p.circle_stroke(c, r, Stroke::new(2.5, st.roles.accent));
            let t = (now / PULSE_SECS).fract() as f32;
            p.circle_stroke(
                c,
                r + 18.0 * t,
                Stroke::new(2.0, alpha(st.roles.accent, (200.0 * (1.0 - t)) as u8)),
            );
            // The pointer stops at the scrim, not inside the circle.
            let bands = [
                Rect::from_min_max(screen.min, pos2(screen.right(), hole.top())),
                Rect::from_min_max(pos2(screen.left(), hole.bottom()), screen.max),
                Rect::from_min_max(
                    pos2(screen.left(), hole.top()),
                    pos2(hole.left(), hole.bottom()),
                ),
                Rect::from_min_max(
                    pos2(hole.right(), hole.top()),
                    pos2(screen.right(), hole.bottom()),
                ),
            ];
            for (k, band) in bands.into_iter().enumerate() {
                ui.interact(
                    band,
                    egui::Id::new(("first-run-scrim", k)),
                    egui::Sense::click_and_drag(),
                );
            }
        });
    ctx.request_repaint();

    // Other modules' faces, as last drawn: the card keeps off them. The spotlit filter's own face
    // counts for little (the card may sit under its knob).
    let faces: Vec<(Rect, f32)> = ui_state
        .hits
        .iter()
        .filter_map(|(k, rect)| {
            let id: ModuleId = k.strip_prefix("module:")?.parse().ok()?;
            Some((*rect, if id == filter { 0.1 } else { 1.0 }))
        })
        .collect();
    let at = card_at(screen, c, r, &faces);
    let mut choice = None;
    egui::Area::new(egui::Id::new("first-run-card"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let card = egui::Frame::new()
                .fill(st.roles.raised)
                .stroke(Stroke::new(1.0, st.roles.line2))
                .corner_radius(10)
                .inner_margin(egui::Margin::same(16))
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(140),
                })
                .show(ui, |ui| {
                    ui.set_width(CARD.x - 32.0);
                    ui.set_min_height(CARD.y - 32.0);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.horizontal(|ui| {
                        kit::label(
                            ui,
                            st,
                            Role::Caption,
                            Tone::Text3,
                            format!("STEP {} OF 4", Step::Knob.number()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            for k in (0..4).rev() {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(12.0, 3.0), egui::Sense::hover());
                                let ink = if k < Step::Knob.number() {
                                    st.roles.accent
                                } else {
                                    st.roles.line2
                                };
                                ui.painter().rect_filled(r, 1.5, ink);
                            }
                        });
                    });
                    kit::label(ui, st, Role::H3, Tone::Text, "Turn this knob");
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text2,
                        "Cutoff sets how bright the sound is. The teal arc is the envelope \
                         opening the filter on every note.",
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let skip = ui.add(kit::Button::new(st, "Skip tour").ghost());
                        ui_state.record("tour-skip".into(), skip.rect);
                        if skip.clicked() {
                            choice = Some(Choice::Skip);
                        }
                        kit::label(ui, st, Role::Caption, Tone::Text3, "Esc");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let next = ui.add(kit::Button::new(st, "Next").primary());
                            ui_state.record("tour-next".into(), next.rect);
                            if next.clicked() {
                                choice = Some(Choice::Next);
                            }
                        });
                    });
                });
            ui_state.record("tour-card".into(), card.response.rect);
        });
    choice
}

/// Where the card's top-left goes, beside the spotlight circle (centre `c`, radius `r`): of the
/// four sides (right, left, below, above; ties go in that order) the one that covers the least
/// of `faces` (a module face and how much it matters), kept inside `screen`, and never over the
/// circle.
fn card_at(screen: Rect, c: Pos2, r: f32, faces: &[(Rect, f32)]) -> Pos2 {
    const GAP: f32 = 22.0;
    const EDGE: f32 = 12.0;
    let clamp = |p: Pos2| {
        pos2(
            p.x.min(screen.right() - CARD.x - EDGE)
                .max(screen.left() + EDGE),
            p.y.min(screen.bottom() - CARD.y - EDGE)
                .max(screen.top() + EDGE),
        )
    };
    let side_y = c.y - r * 0.2;
    let across_x = c.x - CARD.x / 2.0;
    let hole = Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r));
    let area = |r: Rect| {
        if r.is_positive() {
            r.width() * r.height()
        } else {
            0.0
        }
    };
    let mut best = (f32::MAX, clamp(pos2(c.x + r + GAP, side_y)));
    for p in [
        pos2(c.x + r + GAP, side_y),
        pos2(c.x - r - GAP - CARD.x, side_y),
        pos2(across_x, c.y + r + GAP),
        pos2(across_x, c.y - r - GAP - CARD.y),
    ] {
        let at = clamp(p);
        let card = Rect::from_min_size(at, CARD);
        let cost = area(card.intersect(hole)) * 1000.0
            + faces
                .iter()
                .map(|(f, w)| area(card.intersect(*f)) * w)
                .sum::<f32>();
        if cost < best.0 {
            best = (cost, at);
        }
    }
    best.1
}

/// The scrim over `screen` with a round hole of radius `r` at `c`: four bands around the hole's
/// bounding square and a fan in each of its corners.
fn paint_scrim(p: &egui::Painter, screen: Rect, c: Pos2, r: f32) {
    use std::f32::consts::PI;
    let sq = Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r));
    for band in [
        Rect::from_min_max(screen.min, pos2(screen.right(), sq.top())),
        Rect::from_min_max(pos2(screen.left(), sq.bottom()), screen.max),
        Rect::from_min_max(pos2(screen.left(), sq.top()), pos2(sq.left(), sq.bottom())),
        Rect::from_min_max(
            pos2(sq.right(), sq.top()),
            pos2(screen.right(), sq.bottom()),
        ),
    ] {
        if band.is_positive() {
            p.rect_filled(band, 0.0, SCRIM);
        }
    }
    let mut mesh = egui::Mesh::default();
    for (corner, a0) in [
        (sq.left_top(), PI),
        (sq.right_top(), 1.5 * PI),
        (sq.right_bottom(), 0.0),
        (sq.left_bottom(), 0.5 * PI),
    ] {
        const N: usize = 16;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, SCRIM);
        for i in 0..=N {
            let a = a0 + 0.5 * PI * i as f32 / N as f32;
            mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * r, SCRIM);
        }
        for i in 0..N as u32 {
            mesh.add_triangle(base, base + 1 + i, base + 2 + i);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}
