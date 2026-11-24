//! The Start card: lesson 00 (a lesson with `card = true`) docked at the bottom right of the
//! page, with no scrim. The rack behind it stays visible and usable; the step's anchor ring
//! (`mark`) points at the control it talks about.

use egui::{vec2, Color32, Rect, Stroke};
use kabl_modules::builtins::Transport;

use super::all;
use super::run::step_to;
use crate::first_run::devices::Choice;
use crate::kit::{self, Tone};
use crate::style::{Role, Style};
use crate::{PatchEditor, UiState};

const W: f32 = 300.0;
const EDGE: f32 = 16.0;
/// How long the audio output may still be starting before the card says there is none.
const QUIET_SECS: f64 = 2.0;
/// Shown when no audio output runs (no device, or it failed).
pub const NO_OUTPUT: &str = "No sound output: kabl could not open an audio device.";

/// One frame of the Start card while a card lesson runs on its own patch. Esc is Skip. Skip,
/// Close and opening another sound end the lesson; Learn can start it again.
pub fn card(
    editor: &PatchEditor,
    ui_state: &mut UiState,
    ctx: &egui::Context,
    st: &Style,
    page: Rect,
    now: f64,
) {
    let Some((lesson, step, detached, step_done)) = ui_state.lessons.active.as_ref().map(|a| {
        (
            a.lesson,
            a.step,
            a.detached,
            a.done.get(a.step).copied().unwrap_or(false),
        )
    }) else {
        return;
    };
    let l = &all()[lesson];
    if !l.card {
        return;
    }
    if detached {
        ui_state.lessons.active = None;
        return;
    }
    // Esc skips, unless Esc belongs to something else (a dialog, a menu, a text field).
    let free = ui_state.browser.dialog.is_none()
        && !ctx.egui_wants_keyboard_input()
        && !egui::Popup::is_any_open(ctx);
    if free && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        ui_state.lessons.active = None;
        return;
    }
    let quiet = if ui_state.inspect.audio {
        ui_state.lessons.quiet_since = None;
        false
    } else {
        now - *ui_state.lessons.quiet_since.get_or_insert(now) >= QUIET_SECS
    };
    let clocks: Vec<_> = editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let running = clocks
        .iter()
        .any(|id| ui_state.clock_running.get(id).copied().unwrap_or(true));
    let n = l.steps.len();
    let (mut skip, mut next, mut stop) = (false, false, false);
    egui::Area::new(egui::Id::new("start-card"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(page.right_bottom() - vec2(EDGE, EDGE))
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
                    ui.set_width(W - 32.0);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.horizontal(|ui| {
                        let head = if step < n {
                            format!("{} · STEP {} OF {n}", l.title.to_uppercase(), step + 1)
                        } else {
                            l.title.to_uppercase()
                        };
                        kit::label(ui, st, Role::Caption, Tone::Text3, head);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            for k in (0..n).rev() {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(12.0, 3.0), egui::Sense::hover());
                                let ink = if k <= step {
                                    st.roles.accent
                                } else {
                                    st.roles.line2
                                };
                                ui.painter().rect_filled(r, 1.5, ink);
                            }
                        });
                    });
                    if let Some(s) = l.steps.get(step) {
                        if let Some(t) = &s.title {
                            kit::label(ui, st, Role::H3, Tone::Text, t);
                        }
                        // `devices` is `None` only in a plugin.
                        let text = match (&ui_state.devices, &s.plugin_text) {
                            (None, Some(t)) => t,
                            _ => &s.text,
                        };
                        kit::paragraph(ui, st, Role::Body, Tone::Text2, text);
                        if s.sound_check && ui_state.devices.is_some() {
                            sound_check(ui, st, ui_state);
                        }
                        if s.show
                            .as_ref()
                            .is_some_and(|t| super::resolve(editor.state(), t).is_none())
                        {
                            kit::paragraph(
                                ui,
                                st,
                                Role::Caption,
                                Tone::Text3,
                                "Not in this sound. Next goes on.",
                            );
                        }
                        if step_done {
                            kit::label(ui, st, Role::Caption, Tone::Text3, "Done.");
                        }
                    } else {
                        kit::label(ui, st, Role::H3, Tone::Text, "Over to you");
                        kit::paragraph(ui, st, Role::Body, Tone::Text2, &l.end.unguided);
                    }
                    if quiet {
                        let r = kit::paragraph(ui, st, Role::Caption, Tone::Text3, NO_OUTPUT);
                        ui_state.record("start-no-output".into(), r.rect);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let b = ui.add(
                            kit::Button::new(st, if step < n { "Skip" } else { "Close" }).ghost(),
                        );
                        ui_state.record("start-skip".into(), b.rect);
                        skip = b.clicked();
                        kit::label(ui, st, Role::Caption, Tone::Text3, "Esc");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if step < n {
                                let b = ui.add(kit::Button::new(st, "Next").primary());
                                ui_state.record("start-next".into(), b.rect);
                                next = b.clicked();
                            }
                            let b = ui.add(
                                kit::Button::new(st, if running { "Stop" } else { "Play" })
                                    .icon(if running {
                                        kit::Ic::Stop
                                    } else {
                                        kit::Ic::Play
                                    })
                                    .enabled(!clocks.is_empty()),
                            );
                            ui_state.record("start-stop".into(), b.rect);
                            stop = b.clicked();
                        });
                    });
                });
            ui_state.record("start-card".into(), card.response.rect);
        });
    if stop {
        let t = if running {
            Transport::Stop
        } else {
            Transport::Run
        };
        ui_state.transport.extend(clocks.iter().map(|&id| (id, t)));
        // Shown at once; the audio thread confirms it.
        for &id in &clocks {
            ui_state.clock_running.insert(id, !running);
        }
    }
    if next {
        step_to(editor, ui_state, step + 1);
    }
    if skip {
        ui_state.lessons.active = None;
    }
}

/// The Sound-check strip: the output device and MIDI input (applied at once and remembered,
/// as from the status bar) and the output meter. It plays nothing itself.
fn sound_check(ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState) {
    let remembered = ui_state.devices.clone().unwrap_or_default();
    let strip = ui.vertical(|ui| {
        kit::gap(ui, st, 1);
        let row = |ui: &mut egui::Ui, name: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(56.0, 20.0), egui::Sense::hover());
                ui.put(r, |ui: &mut egui::Ui| {
                    kit::label(ui, st, Role::Caption, Tone::Text3, name)
                });
                add(ui);
            });
        };
        let chosen = ui_state
            .audio_output
            .as_ref()
            .map_or("System default".to_string(), |(_, n)| n.clone());
        let mut out = None;
        row(ui, "Output", &mut |ui| {
            let r = kit::dropdown(ui, st, "sound-check-output", &chosen, 204.0, |ui| {
                if kit::menu_item(ui, st, "System default", ui_state.audio_output.is_none())
                    .clicked()
                {
                    out = Some(None);
                }
                for dev in &ui_state.audio_outputs {
                    let on = ui_state.audio_output.as_ref() == Some(dev);
                    if kit::menu_item(ui, st, &dev.1, on).clicked() {
                        out = Some(Some(dev.clone()));
                    }
                }
            });
            ui_state.record("sound-check-output".into(), r.rect);
        });
        if let Choice::Named(n) = &remembered.output {
            if ui_state.audio_output.as_ref().is_none_or(|(_, o)| o != n) {
                kit::paragraph(
                    ui,
                    st,
                    Role::Caption,
                    Tone::Text3,
                    format!("\u{201c}{n}\u{201d} not found: playing on the system default."),
                );
            }
        }
        let port = ui_state
            .midi_input
            .clone()
            .unwrap_or_else(|| "No MIDI input".into());
        let mut midi = None;
        row(ui, "MIDI in", &mut |ui| {
            let r = kit::dropdown(ui, st, "sound-check-midi", &port, 204.0, |ui| {
                if kit::menu_item(ui, st, "No MIDI input", ui_state.midi_input.is_none()).clicked()
                {
                    midi = Some(None);
                }
                for p in &ui_state.midi_inputs {
                    let on = ui_state.midi_input.as_ref() == Some(p);
                    if kit::menu_item(ui, st, p, on).clicked() {
                        midi = Some(Some(p.clone()));
                    }
                }
            });
            ui_state.record("sound-check-midi".into(), r.rect);
        });
        if let (Choice::Named(n), None) = (&remembered.midi, &ui_state.midi_input) {
            kit::paragraph(
                ui,
                st,
                Role::Caption,
                Tone::Text3,
                format!("\u{201c}{n}\u{201d} not found: it connects when plugged in."),
            );
        }
        row(ui, "Level", &mut |ui| {
            let r = crate::record::meter_ui(ui, st, &mut ui_state.meter);
            ui_state.record("sound-check-meter".into(), r.rect);
        });
        if out.is_some() {
            ui_state.output_select = out;
        }
        if midi.is_some() {
            ui_state.midi_select = midi;
        }
    });
    ui_state.record("sound-check".into(), strip.response.rect);
}
