//! The drawer's sequencer and cues editors.

use super::*;

/// The drawer's sequencer header: which bank the controls below edit, bank edits, and the
/// sequencer's own launch settings.
pub(crate) fn seq_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    edit: usize,
) {
    let state = editor.state();
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(state, id), None));
    ui.horizontal_wrapped(|ui| {
        ui.label("Editing");
        for b in 0..seq::BANKS {
            let r = ui.selectable_label(b == edit, banks::title(state, id, b));
            ui_state.record(format!("drawer-bank:{id}.{b}"), r.rect);
            if r.clicked() {
                ui_state.edit_bank.insert(id, b);
            }
        }
    });
    ui.label(
        egui::RichText::new(format!(
            "Playing {}{}{}",
            banks::title(state, id, playing),
            queued.map_or(String::new(), |q| format!(
                " · queued {}",
                banks::title(state, id, q)
            )),
            if banks::startup(state, id) == playing {
                " · startup".to_string()
            } else {
                format!(
                    " · startup {}",
                    banks::title(state, id, banks::startup(state, id))
                )
            }
        ))
        .small(),
    );
    ui.menu_button(format!("Bank {} ↓", seq::BANK_NAMES[edit]), |ui| {
        bank_menu(editor, ui_state, ui, id, edit)
    });
    let (mut pick, mut tpick) = (None, None);
    ui.horizontal_wrapped(|ui| {
        let state = editor.state();
        ui.label("Launch on");
        let clocks: Vec<ModuleId> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "clock")
            .map(|(&c, _)| c)
            .collect();
        let chosen = state
            .modules
            .get(&id)
            .and_then(|m| m.params.get(banks::LAUNCH_CLOCK))
            .map(|&v| v as ModuleId);
        let current = banks::reference_clock(state, id);
        let text = match (current, chosen) {
            (Some(c), Some(_)) => format!("Clock #{c}"),
            (Some(c), None) => format!("Clock #{c} (patched)"),
            (None, _) => "no clock".into(),
        };
        egui::ComboBox::from_id_salt(("seq-clock", id))
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(chosen.is_none(), "The patched clock")
                    .clicked()
                {
                    pick = Some(None);
                }
                for c in clocks {
                    if ui
                        .selectable_label(chosen == Some(c), format!("Clock #{c}"))
                        .clicked()
                    {
                        pick = Some(Some(c as f32));
                    }
                }
            });
        let timing = banks::timing(state, id);
        egui::ComboBox::from_id_salt(("seq-timing", id))
            .selected_text(
                banks::TIMINGS
                    .iter()
                    .find(|t| t.0 == timing)
                    .map_or("", |t| t.1),
            )
            .show_ui(ui, |ui| {
                for (i, (t, name)) in banks::TIMINGS.iter().enumerate() {
                    if ui.selectable_label(*t == timing, *name).clicked() {
                        tpick = Some(i);
                    }
                }
            });
    });
    if let Some(v) = pick {
        editor.set_presentation(&[(id, banks::LAUNCH_CLOCK.into(), v)]);
    }
    if let Some(i) = tpick {
        let v = (i != 2).then_some(i as f32);
        editor.set_presentation(&[(id, banks::LAUNCH_TIMING.into(), v)]);
    }
    ui.separator();
}

/// A cue's Playing (accent fill) or Queued (accent outline) chip.
fn state_chip(ui: &mut egui::Ui, st: &Style, text: &str, solid: bool) {
    let r = &st.roles;
    let font = st.font(style::Role::Label);
    let g = ui.painter().layout_no_wrap(
        text.to_string(),
        font,
        if solid { r.on_accent } else { r.accent_text },
    );
    let (rect, _) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 22.0), Sense::hover());
    if solid {
        ui.painter().rect_filled(rect, 11.0, r.accent);
    } else {
        ui.painter().rect_stroke(
            rect,
            11.0,
            Stroke::new(1.5, r.accent),
            egui::StrokeKind::Inside,
        );
    }
    let c = g.size();
    ui.painter().galley(rect.center() - c / 2.0, g, r.text);
}

/// A label in the drawer's 96 px label column, then `add` beside it.
fn cue_row(ui: &mut egui::Ui, st: &Style, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(84.0, st.metrics.control_h), Sense::hover());
        ui.painter().text(
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            st.font(style::Role::Body),
            st.roles.text2,
        );
        add(ui);
    });
}

/// The drawer's cue editor (G4 `cues-editor`): header, the launch quantise for every cue,
/// one card per cue (number, name, what it sets, Playing / Queued; a tap opens its settings:
/// name, capture, delete, clock, launch, a bank or Keep per sequencer, macros and morph),
/// "Add cue from what plays" and a one-line explainer.
pub(crate) fn cues_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
) {
    let st = ui_state.style.clone();
    let st = &*st;
    let state = editor.state();
    let all = cues::cues(state, id);
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&c, _)| c)
        .collect();
    let seqs: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| matches!(m.kind.as_str(), "seq" | "drum.grid"))
        .map(|(&c, _)| c)
        .collect();
    let is_grid = |s: ModuleId| state.modules.get(&s).is_some_and(|m| m.kind == "drum.grid");
    let playing_now: HashMap<ModuleId, usize> = seqs
        .iter()
        .map(|&s| {
            (
                s,
                ui_state.seq_banks.get(&s).map_or_else(
                    || {
                        if is_grid(s) {
                            state.modules[&s]
                                .params
                                .get("pattern")
                                .map_or(0, |&v| v.round() as usize)
                        } else {
                            banks::startup(state, s)
                        }
                    },
                    |b| b.0,
                ),
            )
        })
        .collect();
    enum Edit {
        Add,
        Delete(usize),
        Capture(usize),
        Clock(usize, ModuleId),
        Timing(usize, kabl_engine::patch_engine::Timing),
        TimingAll(kabl_engine::patch_engine::Timing),
        Target(usize, ModuleId, Option<usize>),
        Rename(usize, String),
        Macro(usize, ModuleId, u8, Option<f32>),
        Morph(usize, f32),
        StoreMacros(usize),
    }
    let macro_mods: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "macro")
        .map(|(&c, _)| c)
        .collect();
    let mut edit = None;
    const SHORT: [&str; 3] = ["Now", "Step", "Bar"];

    // Header: title, kind chip, Explain.
    ui.horizontal(|ui| {
        kit::label(ui, st, style::Role::H3, kit::Tone::Text, "Cues");
        kit::tag(ui, st, &format!("cues #{id}"), st.roles.text2);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.add(
                kit::Button::new(st, "Explain")
                    .ghost()
                    .small()
                    .tone(kit::Tone::Accent),
            );
            ui_state.record(format!("explain-module:{id}"), r.rect);
            if r.clicked() {
                explain::open(ui_state, explain::Subject::Module(id), None);
            }
        });
    });
    ui.add_space(st.sp(2));
    // Launch: the quantise every cue uses (none lit while the cues differ).
    let common = all
        .first()
        .map(|c| c.timing)
        .filter(|t| all.iter().all(|c| c.timing == *t));
    ui.horizontal(|ui| {
        kit::section(ui, st, "Launch");
        kit::segmented(ui, st, |ui| {
            for (i, (t, _)) in banks::TIMINGS.iter().enumerate() {
                let r = kit::seg(ui, st, SHORT[i], common == Some(*t));
                ui_state.record(format!("cue-launch:{id}.{i}"), r.rect);
                if r.clicked() {
                    edit = Some(Edit::TimingAll(*t));
                }
            }
        });
    });
    ui.add_space(st.sp(2));

    let bank_of = |s: ModuleId| {
        ui_state
            .seq_banks
            .get(&s)
            .copied()
            .unwrap_or((banks::startup(state, s), None))
    };
    let last = ui_state.last_cue.get(&id).copied();
    let standing: Vec<(bool, bool)> = all
        .iter()
        .map(|c| cues::standing_now(c, &bank_of, last))
        .collect();
    let open_id = Id::new(("cue-open", id));
    let mut open: Option<usize> = ui.data(|d| d.get_temp(open_id)).flatten();
    let r = &st.roles;
    for (c, &(playing, queued)) in all.iter().zip(&standing) {
        let lit = playing || queued;
        let frame = egui::Frame::new()
            .fill(if playing {
                if st.dark {
                    r.raised
                } else {
                    r.surface
                }
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::new(
                if lit { 1.5 } else { 1.0 },
                if lit { r.accent } else { r.line },
            ))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 10));
        let card = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            let head = ui.horizontal(|ui| {
                kit::label(
                    ui,
                    st,
                    style::Role::Value,
                    kit::Tone::Text2,
                    c.n.to_string(),
                );
                kit::label(ui, st, style::Role::H3, kit::Tone::Text, &c.name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if playing {
                        state_chip(ui, st, "Playing", true);
                    } else if queued {
                        state_chip(ui, st, "Queued", false);
                    }
                });
            });
            // What the cue sets, one phrase per sequencer: "Seq #2 B · Grid #5 keep".
            let mut parts: Vec<String> = seqs
                .iter()
                .map(|&s| {
                    let kind = if is_grid(s) { "Grid" } else { "Seq" };
                    let names: &[&str] = if is_grid(s) {
                        &kabl_modules::builtins::PATTERN_NAMES
                    } else {
                        &seq::BANK_NAMES
                    };
                    let b = c.targets.iter().find(|t| t.0 == s).map(|t| t.1);
                    format!("{kind} #{s} {}", b.map_or("keep", |b| names[b]))
                })
                .collect();
            if !c.macros.is_empty() {
                parts.push(format!(
                    "{} macro{}",
                    c.macros.len(),
                    if c.macros.len() == 1 { "" } else { "s" }
                ));
            }
            ui.add_space(2.0);
            kit::paragraph(
                ui,
                st,
                style::Role::Label,
                kit::Tone::Text2,
                parts.join(" · "),
            );
            head.response.rect
        });
        let head = card.response.rect;
        let resp = ui.interact(head, Id::new(("cue-card", id, c.n)), Sense::click());
        ui_state.record(format!("cue-card:{id}.{}", c.n), head);
        if resp.clicked() {
            open = if open == Some(c.n) { None } else { Some(c.n) };
        }
        if open == Some(c.n) {
            ui.add_space(st.sp(1));
            ui.horizontal(|ui| {
                let mut name = ui_state
                    .cue_rename
                    .as_ref()
                    .filter(|(i, n, _)| (*i, *n) == (id, c.n))
                    .map_or_else(|| c.name.clone(), |(_, _, t)| t.clone());
                let w = (ui.available_width() - 150.0).max(100.0);
                let r = kit::field(ui, st, &mut name, "Name", None, Some(w));
                ui_state.record(format!("cue-name:{id}.{}", c.n), r.rect);
                if r.changed() {
                    ui_state.cue_rename = Some((id, c.n, name.clone()));
                }
                if r.lost_focus() {
                    ui_state.cue_rename = None;
                    if name != c.name {
                        edit = Some(Edit::Rename(c.n, name));
                    }
                }
                let r = ui
                    .add(kit::Button::new(st, "Capture").ghost().small())
                    .on_hover_text("Set every sequencer to the bank it plays now");
                ui_state.record(format!("cue-capture:{id}.{}", c.n), r.rect);
                if r.clicked() {
                    edit = Some(Edit::Capture(c.n));
                }
                let r = ui.add(
                    kit::Button::new(st, "Delete")
                        .ghost()
                        .small()
                        .tone(kit::Tone::Bad),
                );
                ui_state.record(format!("cue-delete:{id}.{}", c.n), r.rect);
                if r.clicked() {
                    edit = Some(Edit::Delete(c.n));
                }
            });
            cue_row(ui, st, "Clock", |ui| {
                let current = c
                    .clock
                    .map_or("choose clock".into(), |k| format!("Clock #{k}"));
                let w = ui.available_width();
                let r = kit::dropdown(ui, st, ("cue-clock", id, c.n), &current, w, |ui| {
                    for &k in &clocks {
                        if kit::menu_item(ui, st, &format!("Clock #{k}"), c.clock == Some(k))
                            .clicked()
                        {
                            edit = Some(Edit::Clock(c.n, k));
                            ui.close();
                        }
                    }
                });
                ui_state.record(format!("cue-clock:{id}.{}", c.n), r.rect);
            });
            cue_row(ui, st, "Launch", |ui| {
                kit::segmented(ui, st, |ui| {
                    for (i, (t, _)) in banks::TIMINGS.iter().enumerate() {
                        let r = kit::seg(ui, st, SHORT[i], c.timing == *t);
                        ui_state.record(format!("cue-timing:{id}.{}.{i}", c.n), r.rect);
                        if r.clicked() {
                            edit = Some(Edit::Timing(c.n, *t));
                        }
                    }
                });
            });
            for &s in &seqs {
                let grid = is_grid(s);
                let label = format!("{} #{s}", if grid { "Grid" } else { "Seq" });
                cue_row(ui, st, &label, |ui| {
                    let current = c.targets.iter().find(|t| t.0 == s).map(|t| t.1);
                    kit::segmented(ui, st, |ui| {
                        let r = kit::seg(ui, st, "Keep", current.is_none())
                            .on_hover_text("This cue leaves it playing");
                        ui_state.record(format!("cue-bank:{id}.{}.{s}.keep", c.n), r.rect);
                        if r.clicked() {
                            edit = Some(Edit::Target(c.n, s, None));
                        }
                        let names: &[&str] = if grid {
                            &kabl_modules::builtins::PATTERN_NAMES
                        } else {
                            &seq::BANK_NAMES
                        };
                        for (b, &name) in names.iter().enumerate() {
                            let hover = if grid {
                                format!("Pattern {name}")
                            } else {
                                banks::title(state, s, b)
                            };
                            let r = kit::seg(ui, st, name, current == Some(b)).on_hover_text(hover);
                            ui_state.record(format!("cue-bank:{id}.{}.{s}.{b}", c.n), r.rect);
                            if r.clicked() {
                                edit = Some(Edit::Target(c.n, s, Some(b)));
                            }
                        }
                    });
                });
            }
            if !macro_mods.is_empty() {
                for &mm in &macro_mods {
                    for k in cues::active_macros(state, mm) {
                        let set = c.macros.iter().find(|t| (t.0, t.1) == (mm, k)).map(|t| t.2);
                        let name = state
                            .label(mm, &format!("name.m{}", k + 1))
                            .map_or_else(|| format!("m{}", k + 1), str::to_string);
                        cue_row(ui, st, &name, |ui| {
                            let mut on = set.is_some();
                            let r = kit::g4::checkbox(ui, st, &mut on, "")
                                .on_hover_text("Tick to set a target for this macro");
                            ui_state.record(format!("cue-macro:{id}.{}.{mm}.{k}", c.n), r.rect);
                            if r.clicked() {
                                let v = set.is_none().then(|| ui_state.macro_now(state, mm, k));
                                edit = Some(Edit::Macro(c.n, mm, k, v));
                            }
                            if let Some(v) = set {
                                let mut x = f64::from(v);
                                let readout = format!("{v:.2}");
                                let d = kit::slider(ui, st, &mut x, 0.0..=1.0, None, &readout);
                                ui_state.record(
                                    format!("cue-macro-value:{id}.{}.{mm}.{k}", c.n),
                                    d.rect,
                                );
                                if d.changed() {
                                    edit = Some(Edit::Macro(c.n, mm, k, Some(x as f32)));
                                }
                            }
                        });
                    }
                }
                cue_row(ui, st, "Morph", |ui| {
                    let mut beats = f64::from(c.morph_beats);
                    let readout = format!("{:.1} beats", c.morph_beats);
                    let w = ui.available_width() - 110.0;
                    ui.allocate_ui(vec2(w.max(80.0), st.metrics.slider_h), |ui| {
                        let d = kit::slider(
                            ui,
                            st,
                            &mut beats,
                            0.0..=f64::from(cues::MAX_MORPH_BEATS),
                            Some(0.5),
                            &readout,
                        )
                        .on_hover_text(
                            "Time the macros take to reach this cue's values, on the clock; 0 jumps",
                        );
                        ui_state.record(format!("cue-morph:{id}.{}", c.n), d.rect);
                        if d.changed() {
                            edit = Some(Edit::Morph(c.n, beats as f32));
                        }
                    });
                    let r = ui
                        .add(kit::Button::new(st, "Store macros").ghost().small())
                        .on_hover_text("Set every macro to where it is now");
                    ui_state.record(format!("cue-store-macros:{id}.{}", c.n), r.rect);
                    if r.clicked() {
                        edit = Some(Edit::StoreMacros(c.n));
                    }
                });
            }
        }
        ui.add_space(st.sp(2));
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    ui.add_space(st.sp(1));
    let w = ui.available_width();
    let r = ui.add(
        kit::Button::new(st, "Add cue from what plays")
            .icon(kit::Ic::Plus)
            .g4(32.0)
            .min_width(w)
            .enabled(all.len() < kabl_modules::builtins::CUES),
    );
    ui_state.record(format!("cue-add:{id}"), r.rect);
    if r.clicked() {
        edit = Some(Edit::Add);
    }
    ui.add_space(st.sp(2));
    kit::paragraph(
        ui,
        st,
        style::Role::Label,
        kit::Tone::Text2,
        "A cue picks a bank for every sequencer and launches them together.",
    );
    ui.add_space(st.sp(3));
    let all_n: Vec<usize> = all.iter().map(|c| c.n).collect();
    match edit {
        Some(Edit::Add) => {
            cues::add(editor, id, &|s| playing_now.get(&s).copied().unwrap_or(0));
        }
        Some(Edit::Delete(n)) => cues::delete(editor, id, n),
        Some(Edit::Capture(n)) => {
            let ops = seqs
                .iter()
                .map(|&s| kabl_core::Op::SetParam {
                    target: kabl_core::ParamTarget::Module {
                        id,
                        param: format!("cue{n}.seq.{s}"),
                    },
                    value: playing_now[&s] as f32,
                })
                .collect();
            editor.edit(ops);
        }
        Some(Edit::Clock(n, k)) => cues::set_clock(editor, id, n, k),
        Some(Edit::Timing(n, t)) => cues::set_timing(editor, id, n, t),
        Some(Edit::TimingAll(t)) => {
            for n in all_n {
                cues::set_timing(editor, id, n, t);
            }
        }
        Some(Edit::Target(n, s, b)) => cues::set_target(editor, id, n, s, b),
        Some(Edit::Rename(n, t)) => cues::rename(editor, id, n, &t),
        Some(Edit::Macro(n, m, k, v)) => cues::set_macro(editor, id, n, m, k, v),
        Some(Edit::Morph(n, b)) => cues::set_morph(editor, id, n, b),
        Some(Edit::StoreMacros(n)) => {
            let state = editor.state().clone();
            cues::store_macros(editor, id, n, &|m, k| ui_state.macro_now(&state, m, k));
        }
        None => {}
    }
}
