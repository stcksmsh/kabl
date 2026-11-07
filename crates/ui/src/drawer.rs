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

/// "Seq Pitch": the module's short name and the port, capitalised.
fn end_label(state: &PatchState, id: ModuleId, port: &str) -> String {
    let kind = state.modules.get(&id).map_or("?", |m| m.kind.as_str());
    let name = match routing::short_name(kind) {
        "?" => registry::info_for(kind).map_or("?", |i| i.name),
        n => n,
    };
    let mut p = port.to_string();
    if let Some(c) = p.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    format!("{name} {p}")
}

/// One row of the cable list: index, signal colour, source, arrow, destination.
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
    kit::row_bg(ui, st, rect, selected, hover);
    let p = ui.painter();
    let ink = |t: Tone| kit::tone(st, t);
    let font = st.font(Role::Caption);
    let mid = rect.center().y;
    let x0 = rect.left() + st.sp(2);
    p.text(
        pos2(x0, mid),
        egui::Align2::LEFT_CENTER,
        format!("{id:02}"),
        font.clone(),
        ink(Tone::Text3),
    );
    let sw = Rect::from_center_size(
        pos2(x0 + st.sp(5) + st.sp(4) / 2.0, mid),
        vec2(st.sp(4), st.sp(1) / 2.0),
    );
    p.rect_filled(sw, 1.0, port_color(th, editor, &c.from));
    let src_x = x0 + st.sp(5) + st.sp(4) + st.sp(2);
    let half = (rect.right() - src_x - st.sp(2)) / 2.0;
    let (PortRef::Module { id: fi, port: fp }, PortRef::Module { id: ti, port: tp }) =
        (&c.from, &c.to)
    else {
        return resp;
    };
    let src = kit::fit_text(ui, st, &end_label(state, *fi, fp), half - st.sp(3));
    let dst = kit::fit_text(
        ui,
        st,
        &end_label(state, *ti, tp),
        half - st.sp(3) - st.sp(1),
    );
    let p = ui.painter();
    let font = st.font(Role::Body);
    p.text(
        pos2(src_x, mid),
        egui::Align2::LEFT_CENTER,
        src,
        font.clone(),
        ink(Tone::Text),
    );
    p.text(
        pos2(src_x + half, mid),
        egui::Align2::LEFT_CENTER,
        "→",
        font.clone(),
        ink(Tone::Text3),
    );
    p.text(
        pos2(src_x + half + st.sp(3), mid),
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
                format!("{} {}", routing::short_name(kind), routing::target_label(p))
            },
        );
        let source = routing::source_label(state, *fi, fp);
        ui.horizontal(|ui| {
            kit::tag(ui, st, &source, port_color(&th, editor, &c.from));
            let (rect, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), st.metrics.control_h),
                Sense::hover(),
            );
            let bar_w = st.sp(6);
            let pct = format!("{:+.0} %", amount * 100.0);
            let p = ui.painter();
            let mid = rect.center().y;
            let track = Rect::from_center_size(
                pos2(rect.right() - st.sp(7) - st.sp(1) - bar_w / 2.0, mid),
                vec2(bar_w, st.sp(1)),
            );
            p.rect_filled(track, 1.0, st.roles.inset);
            let fill = Rect::from_min_size(
                track.min,
                vec2(bar_w * amount.abs().min(1.0), track.height()),
            );
            p.rect_filled(fill, 1.0, st.roles.accent);
            p.text(
                pos2(rect.right(), mid),
                egui::Align2::RIGHT_CENTER,
                pct,
                st.font(Role::Value),
                kit::tone(st, Tone::Text2),
            );
            let w = track.left() - rect.left() - st.sp(2);
            let text = kit::fit_text(ui, st, &target, w);
            ui.painter().text(
                pos2(rect.left() + st.sp(1), mid),
                egui::Align2::LEFT_CENTER,
                text,
                st.font(Role::Body),
                kit::tone(st, Tone::Text),
            );
        });
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
