//! The routing drawer's chrome (G5): the Routing / Explain / Inspect tabs, the patch-wide cable
//! and modulation lists, and the card that says what the selected module does. The selected
//! control's own editor stays in `routing`.
use crate::kit::{self, Tip, Tone};
use crate::style::{Role, Style};
use crate::theme::Theme;
use crate::{explain, port_color, routing, PatchEditor, UiState};
use egui::{pos2, vec2, Align, Layout, Rect, Sense};
use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_modules::registry;

/// Tabs, then the drawer-wide switches. Tab keys: `routing-tab`, `explain-tab`, `inspect-open`.
pub(crate) fn tabs(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    ui.horizontal(|ui| {
        kit::tabs(ui, st, |ui| {
            let explain_on = ui_state.explain.is_open();
            let inspect_on = ui_state.inspect.open;
            let r = kit::seg(ui, st, "Routing", !explain_on && !inspect_on);
            ui_state.record("routing-tab".into(), r.rect);
            if r.clicked() {
                ui_state.explain.close();
                ui_state.inspect.open = false;
            }
            let r = kit::seg(ui, st, "Explain", explain_on)
                .tip(st, "What the selected module does and what moves it");
            ui_state.record("explain-tab".into(), r.rect);
            if r.clicked() {
                if explain_on {
                    ui_state.explain.close();
                } else if let Some(id) = ui_state.selected_module {
                    explain::open(ui_state, explain::Subject::Module(id), None);
                } else {
                    ui_state.last_message = Some("Select a module to explain it".into());
                }
            }
            let r = kit::seg(ui, st, "Inspect", inspect_on)
                .tip(st, "Measure one output and ask why there is no sound");
            ui_state.record("inspect-open".into(), r.rect);
            if r.clicked() {
                ui_state.inspect.open = !inspect_on;
                ui_state.inspect.reveal = !inspect_on;
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if kit::icon_button(ui, st, kit::Ic::Close, false, true)
                .tip(st, "Close the drawer")
                .clicked()
            {
                ui_state.drawer_open = false;
            }
            let on = ui_state.drawer_more;
            let r = ui
                .add(kit::Button::new(st, "More").small().ghost().selected(on))
                .tip(st, "Help, Compare and Learn");
            ui_state.record("drawer-more".into(), r.rect);
            if r.clicked() {
                ui_state.drawer_more = !on;
            }
        });
    });
    if !ui_state.drawer_more {
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = st.sp(1);
        let on = ui_state.explain.help;
        let r = ui
            .add(kit::Button::new(st, "Help").small().ghost().selected(on))
            .tip(st, "Show what each module and control does");
        ui_state.record("help".into(), r.rect);
        if r.clicked() {
            ui_state.explain.help = !on;
        }
        let on = ui_state.compare.open;
        let r = ui
            .add(kit::Button::new(st, "Compare").small().ghost().selected(on))
            .tip(st, "Keep a reference copy of the patch and restore it");
        ui_state.record("compare-open".into(), r.rect);
        if r.clicked() {
            ui_state.compare.open = !on;
            ui_state.compare.reveal = !on;
        }
        let on = ui_state.recipes.open;
        let r = ui
            .add(kit::Button::new(st, "Learn").small().ghost().selected(on))
            .tip(st, "Three short listening recipes (optional)");
        ui_state.record("learn-open".into(), r.rect);
        if r.clicked() {
            ui_state.recipes.open = !on;
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
        kit::label(ui, st, Role::Label, Tone::Text, "Why it sounds like this");
        kit::label(ui, st, Role::Caption, Tone::Text3, info.name);
        kit::paragraph(ui, st, Role::Caption, Tone::Text2, info.explain);
    });
}
