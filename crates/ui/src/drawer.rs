//! The routing drawer's chrome (G5): the Routing / Explain / Inspect tabs, the patch-wide cable
//! and modulation lists, and the card that says what the selected module does. The selected
//! control's own editor stays in `routing`.
use crate::kit::{self, Tip, Tone};
use crate::style::{Role, Style};
use crate::theme::Theme;
use crate::{explain, port_color, routing, PatchEditor, UiState};
use egui::{pos2, vec2, Rect, Sense};
use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_modules::registry;

/// The drawer's tabs (G4): each is one panel that replaces the drawer's body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Routing,
    Inspect,
    Compare,
    Learn,
    Help,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Routing,
        Tab::Inspect,
        Tab::Compare,
        Tab::Learn,
        Tab::Help,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Routing => "Routing",
            Tab::Inspect => "Inspect",
            Tab::Compare => "Compare",
            Tab::Learn => "Learn",
            Tab::Help => "Help",
        }
    }

    /// The hit key of the tab (and of its rail icon with `rail:` in front).
    pub fn key(self) -> &'static str {
        match self {
            Tab::Routing => "routing-tab",
            Tab::Inspect => "inspect-open",
            Tab::Compare => "compare-open",
            Tab::Learn => "learn-open",
            Tab::Help => "help",
        }
    }

    pub fn icon(self) -> kit::Ic {
        match self {
            Tab::Routing => kit::Ic::Cable,
            Tab::Inspect => kit::Ic::Target,
            Tab::Compare => kit::Ic::Compare,
            Tab::Learn => kit::Ic::Learn,
            Tab::Help => kit::Ic::Info,
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Tab::Routing => "Cables and modulation; Explain opens from here",
            Tab::Inspect => "Measure one output and ask why there is no sound",
            Tab::Compare => "Keep a reference copy of the patch and restore it",
            Tab::Learn => "Three short listening recipes (optional)",
            Tab::Help => "Show what each module and control does",
        }
    }
}

/// The tab showing now. The Explain view belongs to Routing (Back returns to it).
pub fn current(ui_state: &UiState) -> Tab {
    if ui_state.recipes.open {
        Tab::Learn
    } else if ui_state.compare.open {
        Tab::Compare
    } else if ui_state.inspect.open {
        Tab::Inspect
    } else if ui_state.explain.help && !ui_state.explain.is_open() {
        Tab::Help
    } else {
        Tab::Routing
    }
}

/// Shows `tab` and closes the others.
pub fn open(ui_state: &mut UiState, tab: Tab) {
    if tab != Tab::Learn {
        // Leaving Learn keeps the patch and its edits, and ends the running recipe.
        ui_state.recipes.active = None;
    }
    ui_state.explain.close();
    ui_state.inspect.open = false;
    ui_state.compare.open = false;
    ui_state.recipes.open = false;
    ui_state.explain.help = false;
    match tab {
        Tab::Routing => {}
        Tab::Inspect => {
            ui_state.inspect.open = true;
            ui_state.inspect.reveal = true;
        }
        Tab::Compare => {
            ui_state.compare.open = true;
            ui_state.compare.reveal = true;
        }
        Tab::Learn => ui_state.recipes.open = true,
        Tab::Help => ui_state.explain.help = true,
    }
}

/// One drawer tab, as the mockup draws it: 12.5 px text (semibold when showing), a 2 px accent
/// underline under the showing one. No padding: five tabs and their 14 px gaps fit the drawer.
fn tab_button(ui: &mut egui::Ui, st: &Style, text: &str, on: bool) -> egui::Response {
    let family = if on {
        crate::style::Font::SansSemibold
    } else {
        crate::style::Font::SansMedium
    };
    let font = egui::FontId::new(12.5, family.family(st.fonts_ready));
    let r = &st.roles;
    let ink = if on { r.text } else { r.text2 };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x, 28.0), Sense::click());
    let ink = if !on && resp.hovered() { r.text } else { ink };
    let (pos, bar) = (
        pos2(rect.left(), rect.center().y - g.size().y / 2.0),
        Rect::from_min_size(
            pos2(rect.left(), rect.bottom() - 2.0),
            vec2(rect.width(), 2.0),
        ),
    );
    ui.painter().galley(pos, g, ink);
    if on {
        ui.painter().rect_filled(bar, 1.0, r.accent);
    }
    resp
}

/// The tab strip: Routing, Inspect, Compare, Learn, Help. Hit keys are `Tab::key`.
pub(crate) fn tabs(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let now = current(ui_state);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        for tab in Tab::ALL {
            let r = tab_button(ui, st, tab.label(), now == tab).tip(st, tab.tip());
            ui_state.record(tab.key().into(), r.rect);
            // The Routing tab also leaves the Explain view, which belongs to it.
            if r.clicked() && (now != tab || tab == Tab::Routing) {
                open(ui_state, tab);
            }
        }
    });
}

/// The mockup's list name for a module kind ("Seq", "VA Osc", "SVF"); the registry name for the
/// rest. Kept apart from `routing::short_name`, which the faces use for their badges.
fn list_name(kind: &str) -> String {
    match kind {
        "seq" => "Seq",
        "osc.va" => "VA Osc",
        "filter.svf" => "SVF",
        "env.adsr" => "ADSR",
        "mixer" => "Mixer",
        "delay" => "Delay",
        "reverb" => "Reverb",
        "out" => "Output",
        _ => return registry::info_for(kind).map_or_else(|| "?".into(), |i| i.name.to_string()),
    }
    .to_string()
}

/// A port name as the list shows it: words capitalised, short filter and CV words upper-case,
/// a trailing digit split off ("in1" is "In 1").
fn port_words(port: &str) -> String {
    let split = port.trim_end_matches(|c: char| c.is_ascii_digit());
    let digits = &port[split.len()..];
    let mut words: Vec<String> = split
        .split(['_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "cv" | "lp" | "bp" | "hp" | "fm" | "wt" | "cc" => w.to_uppercase(),
            _ => {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
        })
        .collect();
    if !digits.is_empty() {
        words.push(digits.to_string());
    }
    words.join(" ")
}

/// "Seq Pitch": the module's list name and the port.
fn end_label(state: &PatchState, id: ModuleId, port: &str) -> String {
    let kind = state.modules.get(&id).map_or("?", |m| m.kind.as_str());
    format!("{} {}", list_name(kind), port_words(port))
}

/// The list's medium-weight sans, for names the eye scans (cables and targets).
fn medium(st: &Style) -> egui::FontId {
    egui::FontId::new(
        st.font(Role::Body).size,
        crate::style::Font::SansMedium.family(st.fonts_ready),
    )
}

/// One row of the cable list: index, signal swatch, source, arrow, destination. The selected
/// row is filled and outlined like a raised card.
fn cable_row(
    ui: &mut egui::Ui,
    st: &Style,
    th: &Theme,
    editor: &PatchEditor,
    id: CableId,
    selected: bool,
) -> egui::Response {
    let state = editor.state();
    let c = &state.cables[&id];
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), st.metrics.row_h), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(
        resp.id.with("hover"),
        resp.hovered(),
        crate::style::Style::secs(st.motion.hover),
    );
    let card = rect.shrink2(vec2(st.sp(1), 0.0));
    if selected {
        let p = ui.painter();
        p.rect_filled(card, st.radius(crate::style::Rad::Md), st.roles.raised);
        p.rect_stroke(
            card,
            st.radius(crate::style::Rad::Md),
            egui::Stroke::new(1.0, st.roles.line),
            egui::StrokeKind::Inside,
        );
    } else {
        kit::row_bg(ui, st, card, false, hover);
    }
    let p = ui.painter();
    let ink = |t: Tone| kit::tone(st, t);
    let mid = rect.center().y;
    let x0 = card.left() + st.sp(1) + st.sp(1) / 2.0;
    p.text(
        pos2(x0, mid),
        egui::Align2::LEFT_CENTER,
        format!("{id:02}"),
        st.font(Role::Value),
        ink(Tone::Text3),
    );
    let sw = Rect::from_min_size(
        pos2(x0 + st.sp(5) - st.sp(1), mid - 2.0),
        vec2(st.sp(4), 4.0),
    );
    p.rect_filled(sw, 2.0, port_color(th, editor, &c.from));
    let (PortRef::Module { id: fi, port: fp }, PortRef::Module { id: ti, port: tp }) =
        (&c.from, &c.to)
    else {
        return resp;
    };
    let src_x = sw.right() + st.sp(2);
    let dst_x = card.left() + card.width() * 0.62;
    let arrow_x = dst_x - st.sp(2) - st.sp(1) / 2.0;
    let src = kit::fit_text(
        ui,
        st,
        &end_label(state, *fi, fp),
        arrow_x - st.sp(2) - src_x,
    );
    let dst = kit::fit_text(
        ui,
        st,
        &end_label(state, *ti, tp),
        card.right() - st.sp(1) - dst_x,
    );
    let p = ui.painter();
    let font = medium(st);
    p.text(
        pos2(src_x, mid),
        egui::Align2::LEFT_CENTER,
        src,
        font.clone(),
        ink(Tone::Text),
    );
    p.text(
        pos2(arrow_x, mid),
        egui::Align2::CENTER_CENTER,
        "→",
        st.font(Role::Body),
        ink(Tone::Text3),
    );
    p.text(
        pos2(dst_x, mid),
        egui::Align2::LEFT_CENTER,
        dst,
        font,
        ink(Tone::Text),
    );
    resp
}

/// The cable list and the modulation list.
pub(crate) fn lists(editor: &PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let th = crate::theme::theme(&ui_state.style);
    let state = editor.state();
    let cables: Vec<CableId> = state
        .cables
        .iter()
        .filter(|(_, c)| {
            matches!(c.from, PortRef::Module { .. }) && matches!(c.to, PortRef::Module { .. })
        })
        .map(|(&id, _)| id)
        .collect();
    let routes: Vec<CableId> = state
        .cables
        .iter()
        .filter(|(_, c)| matches!(c.to, PortRef::Param { .. }))
        .map(|(&id, _)| id)
        .collect();
    if cables.is_empty() && routes.is_empty() {
        kit::paragraph(
            ui,
            st,
            Role::Caption,
            Tone::Text2,
            "No cables yet. Drag from an output jack to an input jack to patch two modules.",
        );
        return;
    }
    kit::section(ui, st, format!("Cables {}", cables.len()));
    ui.spacing_mut().item_spacing.y = 0.0;
    for id in cables {
        let r = cable_row(ui, st, &th, editor, id, ui_state.selected_route == Some(id));
        ui_state.record(format!("cable-row:{id}"), r.rect);
        if r.clicked() {
            ui_state.selected_route = Some(id);
        }
    }
    if routes.is_empty() {
        return;
    }
    kit::gap(ui, st, 1);
    kit::section(ui, st, format!("Modulation {}", routes.len()));
    for id in routes {
        let c = &state.cables[&id];
        let (PortRef::Module { id: fi, port: fp }, PortRef::Param { id: ti, param }) =
            (&c.from, &c.to)
        else {
            continue;
        };
        let amount = c
            .params
            .get("amount")
            .copied()
            .unwrap_or(kabl_engine::compile::DEFAULT_ROUTE_AMOUNT);
        let target = explain::param_of(state, *ti, param).map_or_else(
            || param.clone(),
            |p| {
                let kind = state.modules.get(ti).map_or("?", |m| m.kind.as_str());
                format!("{} {}", list_name(kind), routing::target_label(p))
            },
        );
        let source = crate::macro_name(state, *fi, fp).unwrap_or_else(|| {
            if fp == "out" {
                list_name(state.modules.get(fi).map_or("?", |m| m.kind.as_str()))
            } else {
                end_label(state, *fi, fp)
            }
        });
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), st.metrics.row_h - st.sp(1)),
            Sense::hover(),
        );
        let mid = rect.center().y;
        let chip_font = egui::FontId::new(
            st.font(Role::Caption).size,
            crate::style::Font::SansSemibold.family(st.fonts_ready),
        );
        let chip_text = kit::fit_text(ui, st, &source, st.sp(12));
        let tw = ui
            .painter()
            .layout_no_wrap(
                chip_text.clone(),
                chip_font.clone(),
                kit::tone(st, Tone::Text),
            )
            .size()
            .x;
        let chip = Rect::from_min_size(
            pos2(rect.left() + st.sp(2), mid - 10.0),
            vec2((tw + st.sp(3)).max(44.0), 20.0),
        );
        let pct = format!("{:+.0} %", amount * 100.0).replace('-', "\u{2212}");
        let p = ui.painter();
        p.rect_filled(chip, 10.0, st.roles.inset);
        p.text(
            chip.center(),
            egui::Align2::CENTER_CENTER,
            chip_text,
            chip_font,
            kit::tone(st, Tone::Text),
        );
        let amount_right = rect.right() - st.sp(2);
        let track = Rect::from_min_size(
            pos2(amount_right - 40.0 - st.sp(8), mid - 2.0),
            vec2(40.0, 4.0),
        );
        p.rect_filled(track, 2.0, st.roles.inset);
        p.rect_filled(
            Rect::from_min_size(track.min, vec2(40.0 * amount.abs().min(1.0), 4.0)),
            2.0,
            th.cv,
        );
        p.text(
            pos2(amount_right, mid),
            egui::Align2::RIGHT_CENTER,
            pct,
            st.font(Role::Value),
            kit::tone(st, Tone::Text2),
        );
        let tx = chip.right() + st.sp(2);
        let text = kit::fit_text(ui, st, &target, track.left() - tx - st.sp(2));
        ui.painter().text(
            pos2(tx, mid),
            egui::Align2::LEFT_CENTER,
            text,
            st.font(Role::Body),
            kit::tone(st, Tone::Text),
        );
    }
}

/// "Why it sounds like this": what the selected module does, in its own authored words.
pub(crate) fn why_card(editor: &PatchEditor, ui_state: &UiState, ui: &mut egui::Ui, st: &Style) {
    let Some(info) = ui_state
        .selected_module
        .and_then(|id| editor.state().modules.get(&id))
        .and_then(|m| registry::info_for(&m.kind))
    else {
        return;
    };
    kit::gap(ui, st, 1);
    kit::card(ui, st, |ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            kit::icon(
                ui.painter(),
                kit::Ic::Info,
                r.center(),
                16.0,
                st.roles.accent,
            );
            ui.label(
                egui::RichText::new("Why it sounds like this")
                    .font(egui::FontId::new(
                        st.font(Role::Body).size,
                        crate::style::Font::SansSemibold.family(st.fonts_ready),
                    ))
                    .color(kit::tone(st, Tone::Text)),
            );
        });
        kit::paragraph(ui, st, Role::Body, Tone::Text2, info.explain);
    });
}
