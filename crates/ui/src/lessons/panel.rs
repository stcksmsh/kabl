//! The Learn tab (lesson cards, then the running step) and the anchor ring.

use egui::{CornerRadius, Rect, Stroke, StrokeKind};
use kabl_engine::patch_engine::{Command, MAX_PREVIEW_NOTES};

use super::run::{resolve, start, step_to};
use super::{all, Lesson, Target};
use crate::explain;
use crate::inspect::{self, Sel};
use crate::style::Style;
use crate::{PatchEditor, UiState};

/// How long "Step n done" stays after a step completed by itself.
const COMPLETED_SECS: f64 = 2.5;
/// One outward pulse of the anchor ring.
const PULSE_SECS: f64 = 1.8;

fn weak(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().weak()).wrap());
}

fn para(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(text.into()).wrap());
}

fn target_name(state: &kabl_core::PatchState, t: &Target) -> String {
    match t {
        Target::Control { id, param, .. } => format!("{} {param}", inspect::title(state, *id)),
        Target::Route {
            from, to, param, ..
        } => format!("route #{from} → #{to} {param}"),
        Target::Module { id, .. } => format!("module #{id}"),
    }
}

/// A test note of `secs` seconds on middle C, through the engine's preview (bounded there).
fn test_note(ui_state: &mut UiState, secs: f32) {
    let mut notes = [0u8; MAX_PREVIEW_NOTES];
    notes[0] = 60;
    ui_state.launches.push(Command::Preview {
        notes,
        count: 1,
        velocity: 100,
        blocks: (secs * ui_state.sample_rate / kabl_engine::graph::BLOCK as f32) as u32,
    });
}

/// The rect recorded last frame under `anchor`; a trailing `*` takes the first key it prefixes.
pub fn anchor_rect(ui_state: &UiState, anchor: &str) -> Option<Rect> {
    match anchor.strip_suffix('*') {
        Some(prefix) => ui_state
            .hits
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(_, r)| *r)
            .next(),
        None => ui_state.hits.get(anchor).copied(),
    }
}

/// The step's anchor ring: an accent outline with a slow outward pulse, painted over the page
/// and clipped to it. It takes no input: everything under and around it stays usable.
pub fn mark(ui_state: &UiState, ctx: &egui::Context, st: &Style, page: Rect, now: f64) {
    let Some(a) = ui_state.lessons.active.as_ref().filter(|a| !a.detached) else {
        return;
    };
    if !ui_state.lessons.open {
        return;
    }
    let Some(anchor) = all()[a.lesson]
        .steps
        .get(a.step)
        .and_then(|s| s.anchor.as_deref())
    else {
        return;
    };
    let Some(r) = anchor_rect(ui_state, anchor).filter(|r| r.intersects(page)) else {
        return;
    };
    let r = r.expand(6.0);
    let radius = |r: Rect| CornerRadius::same((r.width().min(r.height()) / 2.0).min(255.0) as u8);
    let p = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("lesson-anchor"),
        ))
        .with_clip_rect(page);
    p.rect_stroke(
        r,
        radius(r),
        Stroke::new(2.5, st.roles.accent),
        StrokeKind::Outside,
    );
    let t = (now / PULSE_SECS).fract() as f32;
    let pulse = r.expand(14.0 * t);
    p.rect_stroke(
        pulse,
        radius(pulse),
        Stroke::new(2.0, st.roles.accent.gamma_multiply(0.8 * (1.0 - t))),
        StrokeKind::Outside,
    );
    ctx.request_repaint();
}

/// The Learn tab.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, now: f64) {
    let st = ui_state.style.clone();
    let kb = |t: String| crate::kit::Button::new(&st, t).g4(28.0);
    crate::kit::label(
        ui,
        &st,
        crate::style::Role::H3,
        crate::kit::Tone::Text,
        "Lessons",
    );
    if let Some(n) = ui_state.lessons.note.clone() {
        para(ui, n);
    }
    let Some(a) = ui_state.lessons.active.clone() else {
        crate::kit::label(
            ui,
            &st,
            crate::style::Role::Body,
            crate::kit::Tone::Text2,
            "Each opens in its own patch. Yours stays as it is.",
        )
        .on_hover_text(
            "Optional. You are asked first if your sound has unsaved changes. Leave any time: \
             your patch stays as it is.",
        );
        ui.add_space(8.0);
        for (i, l) in all().iter().enumerate() {
            let done = ui_state
                .library
                .as_ref()
                .is_some_and(|lib| lib.prefs.lessons.get(&l.id).is_some_and(|p| p.done));
            if card(ui_state, ui, &st, l, done) {
                start(editor, ui_state, i);
            }
            ui.add_space(8.0);
        }
        return;
    };
    let l = &all()[a.lesson];
    ui.label(egui::RichText::new(&l.title).strong());
    if a.detached {
        para(
            ui,
            "Another sound was opened, so these steps no longer apply to the patch in front of \
             you. Your current sound is untouched.",
        );
        ui.horizontal(|ui| {
            let b = ui.add(kb("Open the lesson patch again".into()));
            ui_state.record("lesson-reopen".into(), b.rect);
            if b.clicked() {
                start(editor, ui_state, a.lesson);
            }
        });
        return;
    }
    weak(ui, &l.summary);
    let n = l.steps.len();
    if a.completed_at.is_some_and(|t| now - t < COMPLETED_SECS) && a.step > 0 {
        weak(ui, format!("Step {} done.", a.step));
        ui.ctx().request_repaint();
    }
    if a.step >= n {
        para(ui, &l.end.unguided);
        weak(
            ui,
            "Close keeps your patch and every edit. Save it from Sounds if you like it.",
        );
    } else {
        let s = &l.steps[a.step];
        ui.label(egui::RichText::new(format!("Step {} of {n}", a.step + 1)).small());
        para(ui, &s.text);
        let state = editor.state();
        let resolved = s
            .show
            .as_ref()
            .map(|t| (resolve(state, t), target_name(state, t)));
        if let Some((None, name)) = &resolved {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "{name} is not in the open patch (deleted, or changed). Undo can bring it \
                     back; nothing is guessed from names."
                ),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if let Some((Some(t), _)) = resolved.clone() {
                let b = ui
                    .add(kb("Show".into()))
                    .on_hover_text("Bring it into view (no change to the sound)");
                ui_state.record("lesson-show".into(), b.rect);
                if b.clicked() {
                    explain::go(editor, ui_state, t, now);
                }
                if explain::can_go_back(&ui_state.explain) {
                    let b = ui.add(kb("Back".into()));
                    ui_state.record("lesson-back".into(), b.rect);
                    if b.clicked() {
                        explain::back(ui_state, now);
                    }
                }
            }
            if let Some(p) = &s.inspect {
                let ok = editor
                    .state()
                    .modules
                    .get(&p.id)
                    .is_some_and(|m| m.kind == p.kind);
                let b = ui.add_enabled(ok, kb(format!("Inspect #{} {}", p.id, p.port)));
                ui_state.record("lesson-inspect".into(), b.rect);
                if b.clicked() {
                    ui_state.selected_module = Some(p.id);
                    ui_state.inspect.select(
                        Sel {
                            id: p.id,
                            port: p.port.clone(),
                        },
                        now,
                    );
                }
            }
            if let Some(secs) = s.play_note {
                let b = ui
                    .add(kb(format!("Play test note ({secs:.0} s)")))
                    .on_hover_text("Middle C, same length and velocity every time");
                ui_state.record("lesson-note".into(), b.rect);
                if b.clicked() {
                    test_note(ui_state, secs);
                }
            }
            if s.compare {
                let b = ui.add(kb("Open Compare".into()));
                ui_state.record("lesson-compare".into(), b.rect);
                if b.clicked() {
                    ui_state.compare.open = true;
                    ui_state.compare.reveal = true;
                }
            }
        });
    }
    ui.horizontal(|ui| {
        let b = ui.add_enabled(a.step > 0, kb("Previous".into()).ghost());
        ui_state.record("lesson-prev".into(), b.rect);
        if b.clicked() {
            step_to(editor, ui_state, a.step - 1);
        }
        let b = ui.add_enabled(
            a.step < n,
            kb(if a.step + 1 == n {
                "Try on your own"
            } else {
                "Next"
            }
            .into())
            .primary(),
        );
        ui_state.record("lesson-next".into(), b.rect);
        if b.clicked() {
            step_to(editor, ui_state, a.step + 1);
        }
    });
}

/// A lesson card (g4-surfaces.html Learn card: 72 px, radius 8, title 14/600 at +26, line 12 at
/// +48, step count in mono at the right). True when clicked.
fn card(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style, l: &Lesson, done: bool) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let hover = resp.hovered();
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(8),
        if hover {
            st.roles.line
        } else {
            st.roles.raised
        },
        Stroke::new(1.0, st.roles.line),
        StrokeKind::Inside,
    );
    let semi = crate::style::Font::SansSemibold.family(st.fonts_ready);
    p.text(
        rect.min + egui::vec2(12.0, 26.0 - 14.0 * 0.36),
        egui::Align2::LEFT_CENTER,
        &l.title,
        egui::FontId::new(14.0, semi),
        st.roles.text,
    );
    let mut job = egui::text::LayoutJob::simple(
        l.summary.clone(),
        egui::FontId::new(12.0, st.font(crate::style::Role::Body).family),
        st.roles.text2,
        rect.width() - 24.0,
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    p.galley(
        rect.min + egui::vec2(12.0, 48.0 - galley.size().y / 2.0 - 12.0 * 0.36 + 2.0),
        galley,
        st.roles.text2,
    );
    p.text(
        egui::pos2(rect.max.x - 12.0, rect.min.y + 26.0 - 11.0 * 0.36),
        egui::Align2::RIGHT_CENTER,
        if done {
            format!("done · {} steps", l.steps.len())
        } else {
            format!("{} steps", l.steps.len())
        },
        st.font(crate::style::Role::Value),
        st.roles.text3,
    );
    let resp = resp.on_hover_text(&l.summary);
    ui_state.record(format!("lesson:{}", l.id), rect);
    resp.clicked()
}
