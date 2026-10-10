//! Rack interaction is presentation-only until a drop or face Apply commits one transaction.
use crate::*;
use kabl_core::{CompositeId, Op};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Item {
    Module(ModuleId),
    Composite(CompositeId),
}
#[derive(Clone)]
pub(crate) struct ReturnView {
    scope: Option<CompositeId>,
    pan: EguiVec2,
    zoom: f32,
    module: Option<ModuleId>,
    composite: Option<CompositeId>,
    inspected: Option<(ModuleId, String)>,
    fitted: bool,
    open: BTreeSet<CompositeId>,
    last_inspected: Option<(ModuleId, String)>,
    lanes_shown: Option<(ModuleId, String, usize)>,
    route: Option<CableId>,
}
impl UiState {
    pub(crate) fn enter_composite(&mut self, p: &PatchState, id: CompositeId) {
        if self.scope == Some(id) || !p.composites.contains_key(&id) {
            return;
        }
        self.navigation.push(ReturnView {
            scope: self.scope,
            pan: self.pan,
            zoom: self.zoom,
            module: self.selected_module,
            composite: self.selected_composite,
            inspected: self.inspected.clone(),
            fitted: self.fitted,
            open: self.composites.open.clone(),
            last_inspected: self.last_inspected.clone(),
            lanes_shown: self.lanes_shown.clone(),
            route: self.selected_route,
        });
        self.scope = Some(id);
        self.composites.open.clear();
        let mut next = Some(id);
        while let Some(id) = next {
            self.composites.open.insert(id);
            next = p.composites[&id].parent;
        }
        self.selected_module = None;
        self.selected_composite = None;
        self.inspected = None;
        self.expanded.clear();
        self.moving = None;
        self.fitted = false;
        self.lanes_shown = None;
        self.reveal = None;
    }
    pub(crate) fn back(&mut self) {
        if let Some(v) = self.navigation.pop() {
            self.scope = v.scope;
            self.pan = v.pan;
            self.zoom = v.zoom;
            self.selected_module = v.module;
            self.selected_composite = v.composite;
            self.inspected = v.inspected;
            self.fitted = v.fitted;
            self.expanded.clear();
            self.moving = None;
            self.lanes_shown = v.lanes_shown;
            self.last_inspected = v.last_inspected;
            self.composites.open = v.open;
            self.selected_route = v.route;
            self.reveal = None;
        }
    }
}

pub(crate) fn layout(p: &PatchState, view: &UiState) -> Layout {
    let mut visible = PatchState {
        modules: p.modules.clone(),
        ..Default::default()
    };
    visible.modules.retain(|id, _| match view.scope {
        None => !p.composites.values().any(|c| c.members.contains(id)),
        Some(scope) => p
            .composites
            .get(&scope)
            .is_some_and(|c| c.members.contains(id)),
    });
    // Members of child definitions are never duplicated in their parent rack.
    for (&id, c) in &p.composites {
        if Some(id) != view.scope && c.parent == view.scope {
            for leaf in kabl_core::composite::leaves(p, id) {
                visible.modules.remove(&leaf);
            }
        }
    }
    let mut lay = rack::layout(&visible, &view.view());
    let mut items = Vec::new();
    for m in &mut lay.mods {
        let pos = p.modules[&m.id].pos;
        m.translate(pos2(pos.x, pos.y) - m.face.min);
        items.push((Item::Module(m.id), pos, m.face.width()));
    }
    for (&id, c) in &p.composites {
        if c.parent == view.scope {
            items.push((
                Item::Composite(id),
                c.pos,
                c.panel.as_ref().map_or(240., |p| p.width),
            ));
        }
    }
    items.sort_by(|a, b| {
        (rack::row_of(a.1.y), a.1.x, a.0)
            .partial_cmp(&(rack::row_of(b.1.y), b.1.x, b.0))
            .unwrap()
    });
    lay.bounds = Rect::NOTHING;
    lay.rows = 0;
    let mut right = std::collections::BTreeMap::<usize, f32>::new();
    for (item, pos, width) in items {
        let row = rack::row_of(pos.y);
        let x = pos.x.max(*right.get(&row).unwrap_or(&rack::RACK_X));
        let rect = Rect::from_min_size(pos2(x, rack::row_y(row)), vec2(width, PANEL_H));
        right.insert(row, rect.right());
        lay.rows = lay.rows.max(row + 1);
        lay.bounds = lay.bounds.union(rect);
        match item {
            Item::Module(id) => {
                let m = lay.mods.iter_mut().find(|m| m.id == id).unwrap();
                m.translate(rect.min - m.face.min);
                m.row = row;
            }
            Item::Composite(id) => {
                lay.faces.insert(id, rect);
            }
        }
    }
    lay
}
pub(crate) fn items(lay: &Layout) -> Vec<(Item, Rect)> {
    lay.mods
        .iter()
        .map(|m| (Item::Module(m.id), m.face))
        .chain(lay.faces.iter().map(|(&id, &r)| (Item::Composite(id), r)))
        .collect()
}
/// Stable insertion slots use neighbour centres; only drop commits the resolved target row.
pub(crate) fn drop_plan(lay: &Layout, item: Item, live: Pos2, width: f32) -> Vec<(Item, Rect)> {
    let row = rack::row_of(live.y);
    let snapped = rack::snap(live);
    let mut neighbours: Vec<_> = items(lay)
        .into_iter()
        .filter(|(i, r)| *i != item && rack::row_of(r.top()) == row)
        .collect();
    neighbours.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    let index = neighbours
        .iter()
        .position(|(_, r)| live.x + width / 2. < r.center().x)
        .unwrap_or(neighbours.len());
    let left = if index == 0 {
        rack::RACK_X
    } else {
        neighbours[index - 1].1.right()
    };
    let right = neighbours
        .get(index)
        .map_or(f32::INFINITY, |(_, r)| r.left());
    let x = if right - left >= width {
        snapped.x.clamp(left, right - width)
    } else {
        right.max(left)
    };
    neighbours.insert(
        index,
        (
            item,
            Rect::from_min_size(pos2(x, rack::row_y(row)), vec2(width, PANEL_H)),
        ),
    );
    let mut next = rack::RACK_X;
    for (_, r) in &mut neighbours {
        let x = r.left().max(next);
        *r = r.translate(vec2(x - r.left(), 0.));
        next = r.right();
    }
    neighbours
}
fn commit_drop(editor: &mut PatchEditor, plan: &[(Item, Rect)]) {
    let mut ops = Vec::new();
    for &(item, r) in plan {
        let pos = Vec2 {
            x: r.left(),
            y: r.top(),
        };
        match item {
            Item::Module(id) if editor.state().modules[&id].pos != pos => {
                ops.push(Op::MoveModule { id, pos })
            }
            Item::Composite(id) if editor.state().composites[&id].pos != pos => {
                let mut value = editor.state().composites[&id].clone();
                value.pos = pos;
                ops.push(Op::SetComposite {
                    id,
                    value: Some(value),
                });
            }
            _ => {}
        }
    }
    if !ops.is_empty() {
        editor.restore_to(ops);
    }
}
pub(crate) fn update_drag(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &egui::Ui,
    lay: &Layout,
) {
    let Some(mut moving) = view.moving.take() else {
        return;
    };
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        moving.cancelled = true;
    }
    if !moving.cancelled {
        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
            // Edge scrolling changes the camera, never the grab offset or the document.
            let c = view.canvas;
            let edge = 28.;
            let step = ui.input(|i| i.stable_dt).min(0.05) * 360.;
            if pointer.y > c.bottom() - edge {
                view.pan.y -= step;
            } else if pointer.y < c.top() + edge {
                view.pan.y += step;
            }
            if pointer.x > c.right() - edge {
                view.pan.x -= step;
            } else if pointer.x < c.left() + edge {
                view.pan.x += step;
            }
            let p = view.xf().inv(pointer) - moving.grab;
            moving.live = Vec2 { x: p.x, y: p.y };
        }
    }
    if ui.input(|i| i.pointer.any_released()) {
        if !moving.cancelled {
            let plan = drop_plan(
                lay,
                moving.item,
                pos2(moving.live.x, moving.live.y),
                moving.width,
            );
            if plan
                .iter()
                .any(|(i, r)| *i == moving.item && r.min != moving.origin)
            {
                commit_drop(editor, &plan);
            }
        }
    } else {
        view.moving = Some(moving);
        ui.ctx().request_repaint();
    }
}
pub(crate) fn body(
    view: &mut UiState,
    ui: &mut egui::Ui,
    item: Item,
    face: Rect,
    xf: Xf,
) -> egui::Response {
    let r = ui.interact(
        xf.r(face),
        Id::new(("rack-item", item)),
        Sense::click_and_drag(),
    );
    if r.clicked() || r.drag_started() {
        match item {
            Item::Module(id) => {
                view.selected_module = Some(id);
                view.selected_composite = None;
            }
            Item::Composite(id) => {
                view.selected_composite = Some(id);
                view.selected_module = None;
            }
        }
    }
    if r.drag_started() && view.moving.is_none() && view.expanded.is_empty() {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            view.moving = Some(Moving {
                item,
                grab: xf.inv(p) - face.min,
                live: Vec2 {
                    x: face.left(),
                    y: face.top(),
                },
                origin: face.min,
                width: face.width(),
                cancelled: false,
            });
        }
    }
    r
}

pub(crate) fn boundary(editor: &mut PatchEditor, view: &mut UiState, ui: &mut egui::Ui) {
    let Some(id) = view.scope else { return };
    let Some(c) = editor.state().composites.get(&id).cloned() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        let r = ui.button("Back");
        view.record("subpatch:back".into(), r.rect);
        if r.clicked() {
            view.back();
        }
        let mut names = vec![c.name.clone()];
        let mut parent = c.parent;
        while let Some(id) = parent {
            let c = &editor.state().composites[&id];
            names.push(c.name.clone());
            parent = c.parent;
        }
        names.reverse();
        ui.strong(format!("Rack / {}", names.join(" / ")));
        ui.label("This instance · exposed boundary ports:");
        for (&key, e) in &c.ports {
            let dir =
                composites::direction(editor.state(), &e.target).unwrap_or(PortDirection::Input);
            let r = ui.button(format!(
                "{} {}",
                if dir == PortDirection::Output {
                    "OUT"
                } else {
                    "IN"
                },
                e.label
            ));
            view.record(format!("boundary:{id}:{key}"), r.rect);
            if r.clicked() {
                on_port_click(editor, view, e.target.clone(), dir);
            }
        }
    });
    let external = external_dependencies(editor.state(), id);
    if !external.is_empty() {
        ui.label(format!(
            "External dependencies remain in parent rack: {}",
            external.join(", ")
        ));
    }
}

fn external_dependencies(p: &PatchState, id: CompositeId) -> Vec<String> {
    let leaves = kabl_core::composite::leaves(p, id);
    let mut deps = BTreeSet::new();
    for &mid in p.modules.keys() {
        if leaves.contains(&mid) {
            if let Some(clock) = banks::reference_clock(p, mid).filter(|c| !leaves.contains(c)) {
                deps.insert(format!("clock #{clock}"));
            }
        }
        for cue in cues::cues(p, mid) {
            if leaves.contains(&mid) {
                if let Some(clock) = cue.clock.filter(|c| !leaves.contains(c)) {
                    deps.insert(format!("cue {} uses clock #{clock}", cue.name));
                }
                for (seq, _) in cue.targets.iter().filter(|(seq, _)| !leaves.contains(seq)) {
                    deps.insert(format!("cue {} targets sequencer #{seq}", cue.name));
                }
            } else if cue.targets.iter().any(|(seq, _)| leaves.contains(seq)) {
                deps.insert(format!(
                    "cue {} on outside module #{mid} launches these internals",
                    cue.name
                ));
            }
        }
    }
    deps.into_iter().collect()
}

/// Keep a stable top-left origin; Area's previous content size must not cap new content.
pub(crate) fn focused_area(ctx: &egui::Context, id: Id, width: f32) -> egui::Area {
    let bounds = ctx.content_rect().shrink(12.);
    let pos = egui::AreaState::load(ctx, id)
        .map(|s| s.left_top_pos())
        .unwrap_or(pos2(bounds.center().x - width * 0.5, bounds.top() + 36.));
    egui::Area::new(id)
        .kind(egui::UiKind::Modal)
        .order(egui::Order::Foreground)
        .sense(Sense::hover())
        .fixed_pos(pos)
        .constrain_to(bounds)
}
pub(crate) fn focused_height(ui: &mut egui::Ui) {
    ui.set_max_height((ui.ctx().content_rect().bottom() - ui.min_rect().top() - 24.).max(64.));
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn more(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    th: &Theme,
    xf: Xf,
    scene: &Layout,
    drawn: &mut Drawn,
) {
    let Some(id) = view.expanded.iter().next().copied() else {
        return;
    };
    let Some(state) = editor.state().modules.get(&id) else {
        view.expanded.clear();
        return;
    };
    let Some(info) = registry::info_for(&state.kind) else {
        return;
    };
    let full = rack::layout(
        editor.state(),
        &rack::View {
            expanded: Some(&view.expanded),
            float: true,
            edit_banks: Some(&view.edit_view),
            skins: view.skins,
            ..Default::default()
        },
    );
    let Some(mut m) = full.get(id).cloned() else {
        return;
    };
    if let Some(adv) = m.adv.as_mut() {
        let bottom = m
            .ctls
            .iter()
            .filter(|c| !c.primary)
            .map(|c| c.geo.label_pos().y + 64.)
            .fold(adv.bottom(), f32::max);
        adv.max.y = bottom;
    }
    let ctx = ui.ctx().clone();
    let mut close = false;
    let width =
        (m.full().width() * xf.zoom.clamp(0.85, 1.0) + 24.).min(ctx.content_rect().width() - 80.);
    let modal = egui::Modal::new(Id::new("more-controls"))
        .area(focused_area(&ctx, Id::new("more-controls"), width))
        .show(&ctx, |ui| {
            focused_height(ui);
            let z = xf.zoom.clamp(0.85, 1.0);
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.heading(format!("More controls · {}", info.name));
                let r = ui.button("Close");
                view.record("more:close".into(), r.rect);
                close = r.clicked();
            });
            ui.label("Original knobs, selectors, jacks and routing. Rack layout stays unchanged.");
            egui::ScrollArea::both()
                .max_height(ui.available_height())
                .show_owned(ui, |ui| {
                    let (_, space) = ui.allocate_space(m.full().size() * z);
                    view.record(format!("more:{id}:body"), space.intersect(ui.clip_rect()));
                    let focus = Xf {
                        origin: space.min - m.full().min.to_vec2() * z,
                        zoom: z,
                    };
                    let painter = ui.painter_at(space.intersect(ui.clip_rect()));
                    let material = theme::panel_theme(&view.style, info.kind);
                    let mut own = Drawn::default();
                    let now = ui.input(|i| i.time);
                    draw_module(editor, view, ui, &painter, th, focus, &m, now, &mut own);
                    if let Some(adv) = m.adv {
                        theme::satin(&painter, focus.r(adv), &material);
                    }
                    let look = Look {
                        z,
                        ink: material.ink,
                        ink2: material.ink2,
                        slot: 76.0,
                    };
                    for c in m.ctls.iter().filter(|c| !c.primary) {
                        draw_control(
                            editor, view, ui, &painter, &material, focus, &m, c, look, false, now,
                            &mut own,
                        );
                    }
                    let mut both = Drawn {
                        ports: drawn.ports.clone(),
                        plugs: own.plugs.clone(),
                        drop_at: None,
                    };
                    both.ports.extend(own.ports.clone());
                    draw_cables(editor, view, ui, &painter, th, xf, scene, &both, Some(id));
                    painter.extend(std::mem::take(&mut view.deferred));
                    drawn.ports.extend(own.ports);
                    drawn.plugs.extend(own.plugs);
                    if own.drop_at.is_some() {
                        drawn.drop_at = own.drop_at;
                    }
                    let entry = egui::CollapsingHeader::new("Keyboard / exact value entry").show(
                        ui,
                        |ui| {
                            for c in &m.ctls {
                                ui.horizontal(|ui| {
                                    ui.label(routing::param_label(c.param));
                                    perform::param_editor(
                                        editor,
                                        view,
                                        ui,
                                        id,
                                        c.param,
                                        false,
                                        Some(format!("more:value:{id}:{}", c.param.name)),
                                    );
                                });
                            }
                        },
                    );
                    view.record("more:exact".into(), entry.header_response.rect);
                });
        });
    view.record("more:dialog".into(), modal.response.rect);
    if close || (modal.should_close() && view.drag.is_none()) {
        view.expanded.clear();
        view.last_inspected = view.inspected.clone();
        view.lanes_shown = inspected_extent(editor, view, scene).map(|(_, key)| key);
        view.reveal = None;
    }
}

pub(crate) fn edit_face(editor: &mut PatchEditor, view: &mut UiState, ctx: &egui::Context) {
    let Some((id, mut visible)) = view.choose.take() else {
        return;
    };
    let Some(m) = editor.state().modules.get(&id) else {
        return;
    };
    let Some(info) = registry::info_for(&m.kind) else {
        return;
    };
    let mut order = view
        .face_order
        .take()
        .unwrap_or_else(|| rack::control_order(m, info));
    let mut done = false;
    let mut cancel = false;
    egui::Window::new(format!("Edit face · {}",info.name)).id(Id::new("native-edit-face")).default_width(420.).default_height(280.).max_height(ctx.content_rect().height()-100.).default_pos(ctx.content_rect().center()-vec2(210.,180.)).collapsible(false).vscroll(true).show(ctx,|ui|{
        ui.label("Normally visible controls. Hidden controls remain in More controls. Order applies within knob/selector rows.");
        for n in 0..order.len() {
            let index=order[n];let p=&info.params[index];if rack::face_name(info,p.name)!=p.name {continue;}
            ui.horizontal(|ui|{
                let r=ui.checkbox(&mut visible[index],routing::param_label(p));view.record(format!("face:{id}:{}",p.name),r.rect);
                let r=ui.add_enabled(n>0,egui::Button::new("↑"));view.record(format!("face:up:{id}:{}",p.name),r.rect);if r.clicked(){order.swap(n,n-1);}
                let r=ui.add_enabled(n+1<order.len(),egui::Button::new("↓"));view.record(format!("face:down:{id}:{}",p.name),r.rect);if r.clicked(){order.swap(n,n+1);}
            });
        }
        ui.horizontal(|ui|{
            let r=ui.button("Apply / Undoable");view.record("face:apply".into(),r.rect);done=r.clicked();
            if ui.button("Restore defaults").clicked(){visible=info.params.iter().map(|p|!info.advanced.contains(&rack::face_name(info,p.name))).collect();order=(0..info.params.len()).collect();}
            cancel=ui.button("Cancel").clicked()||ui.input(|i|i.key_pressed(egui::Key::Escape));
        });
    });
    if done {
        let before = editor.state().clone();
        let mut candidate = before.clone();
        let module = candidate.modules.get_mut(&id).unwrap();
        module
            .params
            .retain(|k, _| !k.starts_with(rack::FACE_PREFIX));
        for (i, p) in info.params.iter().enumerate() {
            let name = rack::face_name(info, p.name);
            if name != p.name {
                continue;
            }
            let default = !info.advanced.contains(&name);
            if visible[i] != default {
                module
                    .params
                    .insert(rack::face_key(name), if visible[i] { 1. } else { 0. });
            }
        }
        if order.iter().copied().ne(0..info.params.len()) {
            for (rank, &index) in order.iter().enumerate() {
                let name = rack::face_name(info, info.params[index].name);
                if name == info.params[index].name && rank != index {
                    module
                        .params
                        .insert(format!("face.order.{name}"), rank as f32);
                }
            }
        }
        if module.params.len() > 256 {
            view.last_message=Some("Face settings exceed per-module saved parameter limit (256). Restore defaults or remove custom settings.".into());
            view.choose = Some((id, visible));
            view.face_order = Some(order);
            return;
        }
        if let Some(ops) = compare::diff(&before, &candidate) {
            editor.restore_to(ops);
        }
    } else if !cancel {
        view.choose = Some((id, visible));
        view.face_order = Some(order);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PatchEditor, UiState, ModuleId, CompositeId) {
        let mut e = PatchEditor::new();
        let a = e.add_module("osc.va", Vec2 { x: 24., y: 10. });
        let b = e.add_module("filter.svf", Vec2 { x: 240., y: 10. });
        let c =
            composites::encapsulate(&mut e, BTreeSet::from([b]), BTreeSet::new(), "Voice").unwrap();
        let mut v = UiState::default();
        v.validate(&e);
        (e, v, a, c)
    }
    fn frame(
        ctx: &egui::Context,
        e: &mut PatchEditor,
        v: &mut UiState,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        frame_size(ctx, e, v, events, vec2(1440., 900.))
    }
    fn frame_size(
        ctx: &egui::Context,
        e: &mut PatchEditor,
        v: &mut UiState,
        events: Vec<egui::Event>,
        size: EguiVec2,
    ) -> egui::FullOutput {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            Id::new("repair-test"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(e, v, &mut root);
        ctx.end_pass()
    }
    fn button(p: Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn click(ctx: &egui::Context, e: &mut PatchEditor, v: &mut UiState, key: &str) {
        let p = v.hits[key].center();
        frame(
            ctx,
            e,
            v,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        frame(ctx, e, v, vec![button(p, false)]);
        frame(ctx, e, v, vec![]);
    }
    #[test]
    fn real_default_child_wheel_never_pans_rack_but_outside_pan_and_zoom_work() {
        let (mut e, mut v, _, composite) = fixture();
        let mut c = e.state().composites[&composite].clone();
        let leaf = *c.members.first().unwrap();
        for key in 1..=20 {
            c.controls.insert(
                key,
                kabl_core::composite::Exposure {
                    label: format!("Control {key}"),
                    target: PortRef::Param {
                        id: leaf,
                        param: "cutoff_hz".into(),
                    },
                },
            );
        }
        e.restore_to(vec![Op::SetComposite {
            id: composite,
            value: Some(c),
        }]);
        let ctx = egui::Context::default();
        for _ in 0..4 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let p = v.hits[&format!("composite:{composite}:control:1")].center();
        let initial = (v.pan, v.zoom);
        let before = v.hits[&format!("composite:{composite}:control:1")];
        let event = |delta, modifiers| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0., delta),
            phase: egui::TouchPhase::Move,
            modifiers,
        };
        frame(&ctx, &mut e, &mut v, vec![egui::Event::PointerMoved(p)]);
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![event(-80., egui::Modifiers::NONE)],
        );
        frame(&ctx, &mut e, &mut v, vec![]);
        assert_ne!(v.hits[&format!("composite:{composite}:control:1")], before);
        assert_eq!((v.pan, v.zoom), initial);
        for _ in 0..12 {
            frame(
                &ctx,
                &mut e,
                &mut v,
                vec![event(-500., egui::Modifiers::NONE)],
            );
        }
        assert_eq!((v.pan, v.zoom), initial, "child bottom boundary owns wheel");
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![event(-80., egui::Modifiers::CTRL)],
        );
        assert_eq!((v.pan, v.zoom), initial, "child owns modifier wheel");
        let outside = v.canvas.right_top() + vec2(-20., 30.);
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(outside)],
        );
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![event(-80., egui::Modifiers::NONE)],
        );
        assert_ne!(v.pan, initial.0);
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![event(80., egui::Modifiers::CTRL)],
        );
        assert_ne!(v.zoom, initial.1);
    }

    #[test]
    fn focused_content_grows_then_scrolls_only_at_viewport_bound() {
        for size in [vec2(1440., 900.), vec2(1280., 800.)] {
            let (mut e, mut v, id, _) = fixture();
            let ctx = egui::Context::default();
            v.expanded.insert(id);
            for _ in 0..5 {
                frame_size(&ctx, &mut e, &mut v, vec![], size);
            }
            let before = v.hits["more:dialog"];
            let rack = (v.pan, v.zoom, v.selected_module, v.selected_composite);
            let p = v.hits["more:exact"].center();
            frame_size(
                &ctx,
                &mut e,
                &mut v,
                vec![egui::Event::PointerMoved(p), button(p, true)],
                size,
            );
            frame_size(&ctx, &mut e, &mut v, vec![button(p, false)], size);
            for _ in 0..8 {
                frame_size(&ctx, &mut e, &mut v, vec![], size);
            }
            let expanded = v.hits["more:dialog"];
            assert!(
                expanded.height() > before.height() + 60.,
                "{before:?} vs {expanded:?}"
            );
            assert_eq!(expanded.min, before.min);
            assert!(expanded.bottom() <= size.y - 8.);
            assert!(expanded.contains_rect(v.hits["more:close"]));
            assert_eq!(
                rack,
                (v.pan, v.zoom, v.selected_module, v.selected_composite)
            );
        }
    }

    #[test]
    fn genuinely_tall_exact_content_is_bounded_and_last_entry_scrolls_into_reach() {
        let (mut e, mut v, _, _) = fixture();
        let id = e.add_module("seq", Vec2 { x: 600., y: 10. });
        let ctx = egui::Context::default();
        let size = vec2(1280., 800.);
        v.expanded.insert(id);
        for _ in 0..5 {
            frame_size(&ctx, &mut e, &mut v, vec![], size);
        }
        let p = v.hits["more:exact"].center();
        frame_size(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p), button(p, true)],
            size,
        );
        frame_size(&ctx, &mut e, &mut v, vec![button(p, false)], size);
        for _ in 0..8 {
            frame_size(&ctx, &mut e, &mut v, vec![], size);
        }
        let dialog = v.hits["more:dialog"];
        assert!(dialog.bottom() <= size.y - 8.);
        let prefix = format!("more:value:{id}:");
        let last = v
            .hits
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .max_by(|(_, a), (_, b)| a.bottom().total_cmp(&b.bottom()))
            .map(|(k, r)| (k.clone(), *r))
            .unwrap();
        assert!(
            last.1.bottom() > dialog.bottom(),
            "tall content genuinely needs scrolling"
        );
        let camera = (v.pan, v.zoom);
        let p = pos2(dialog.left() + 30., dialog.bottom() - 40.);
        frame_size(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p)],
            size,
        );
        frame_size(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0., -5000.),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }],
            size,
        );
        for _ in 0..3 {
            frame_size(&ctx, &mut e, &mut v, vec![], size);
        }
        assert!(
            v.hits["more:dialog"].contains_rect(v.hits[&last.0]),
            "dialog {:?}, last {:?}",
            v.hits["more:dialog"],
            v.hits[&last.0]
        );
        assert!(v.hits["more:dialog"].contains_rect(v.hits["more:close"]));
        assert_eq!((v.pan, v.zoom), camera);
    }

    #[test]
    fn occupied_insertion_is_drawn_above_stationary_panel_below_ghost_and_matches_drop() {
        let (mut e, mut v, id, _) = fixture();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let before = e.state().clone();
        let p = v.hits[&format!("module:{id}")].left_top() + vec2(15., 20.);
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        let to = p + vec2(210., 25.);
        let mut output = frame(&ctx, &mut e, &mut v, vec![egui::Event::PointerMoved(to)]);
        for _ in 0..3 {
            output = frame(&ctx, &mut e, &mut v, vec![egui::Event::PointerMoved(to)]);
        }
        let preview = v.hits["rack:drop-preview"];
        let sel = theme(&v.style).sel;
        let index = output.shapes.iter().position(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r) if r.rect == preview && r.stroke.width == 2. && r.stroke.color == sel)).expect("visible insertion stroke");
        let neighbour = composite_layout(e.state(), &v)
            .faces
            .values()
            .next()
            .copied()
            .unwrap();
        let neighbour = v.xf().r(neighbour);
        assert!(preview.intersects(neighbour), "occupied destination");
        let stationary = output.shapes.iter().position(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.text().contains("Voice · #"))).expect("stationary panel");
        assert!(index > stationary, "preview painted over stationary panel");
        let live = v.moving.as_ref().unwrap();
        let ghost = v.xf().r(Rect::from_min_size(
            pos2(live.live.x, live.live.y),
            vec2(live.width, PANEL_H),
        ));
        let lifted = output
            .shapes
            .iter()
            .rposition(|s| matches!(&s.shape, egui::epaint::Shape::Rect(r) if r.rect == ghost))
            .expect("foreground ghost");
        assert!(lifted > index, "ghost stays above insertion");
        frame(&ctx, &mut e, &mut v, vec![button(to, false)]);
        let placed = composite_layout(e.state(), &v);
        assert_eq!(v.xf().r(placed.get(id).unwrap().face), preview);
        e.undo();
        assert_eq!(e.state(), &before);
    }

    #[test]
    fn dropped_composite_moves_only_own_position_one_undo() {
        let (mut e, v, _, id) = fixture();
        let before = e.state().clone();
        let lay = layout(e.state(), &v);
        let plan = drop_plan(&lay, Item::Composite(id), pos2(100., rack::row_y(2)), 240.);
        commit_drop(&mut e, &plan);
        assert_eq!(e.state().modules, before.modules);
        assert_eq!(e.state().cables, before.cables);
        assert_eq!(e.state().composites[&id].pos.y, rack::row_y(2));
        e.undo();
        assert_eq!(e.state(), &before);
    }
    #[test]
    fn insertion_is_legal_stable_and_matches_committed_layout() {
        let (mut e, v, a, id) = fixture();
        let lay = layout(e.state(), &v);
        let plan = drop_plan(&lay, Item::Composite(id), pos2(-100., 10.), 240.);
        for pair in plan.windows(2) {
            assert!(pair[0].1.right() <= pair[1].1.left());
        }
        assert_eq!(
            plan,
            drop_plan(&lay, Item::Composite(id), pos2(-99., 11.), 240.)
        );
        commit_drop(&mut e, &plan);
        let placed = layout(e.state(), &v);
        for (item, r) in plan {
            let actual = match item {
                Item::Module(id) => placed.get(id).unwrap().face,
                Item::Composite(id) => placed.faces[&id],
            };
            assert_eq!(r, actual);
        }
        assert!(placed.get(a).is_some());
    }
    #[test]
    fn nested_navigation_and_scoped_add_restore_outer_view() {
        let (mut e, mut v, a, id) = fixture();
        v.pan = vec2(30., 70.);
        v.zoom = 0.7;
        v.selected_module = Some(a);
        v.fitted = true;
        let outer = layout(e.state(), &v);
        v.enter_composite(e.state(), id);
        assert!(layout(e.state(), &v).get(a).is_none());
        let before = e.state().clone();
        let added = e.add_module_in_scope("lfo", Vec2 { x: 500., y: 10. }, Some(id));
        assert!(e.state().composites[&id].members.contains(&added));
        e.undo();
        assert_eq!(e.state(), &before);
        v.back();
        assert_eq!(v.pan, vec2(30., 70.));
        assert_eq!(v.zoom, 0.7);
        assert_eq!(v.selected_module, Some(a));
        assert_eq!(layout(e.state(), &v).bounds, outer.bounds);
    }
    #[test]
    fn more_and_edit_drafts_never_reflow_outer_rack() {
        let (e, mut v, a, _) = fixture();
        let original = layout(e.state(), &v);
        v.expanded.insert(a);
        v.choose = Some((
            a,
            vec![true; registry::info_for("osc.va").unwrap().params.len()],
        ));
        let expanded = layout(e.state(), &v);
        assert_eq!(original.bounds, expanded.bounds);
        for m in original.mods {
            assert_eq!(m.face, expanded.get(m.id).unwrap().face);
        }
    }
    #[test]
    fn actual_knob_owns_pointer_and_escape_cancels_item() {
        let (mut e, mut v, a, _) = fixture();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let pan = v.pan;
        let before = e.state().clone();
        let p = v.hits[&format!("knob:{a}.base_hz")].center();
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p - vec2(0., 30.))],
        );
        assert!(v.moving.is_none());
        assert_eq!(v.pan, pan);
        frame(&ctx, &mut e, &mut v, vec![button(p, false)]);
        assert_eq!(e.state().modules[&a].pos, before.modules[&a].pos);
        let rect = v.hits[&format!("module:{a}")];
        let p = rect.left_top() + vec2(15., 20.);
        let stored = e.state().clone();
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::PointerMoved(p + vec2(100., 100.))],
        );
        assert!(v.moving.is_some());
        assert_eq!(e.state(), &stored);
        frame(
            &ctx,
            &mut e,
            &mut v,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&ctx, &mut e, &mut v, vec![button(p, false)]);
        assert_eq!(e.state(), &stored);
    }
    #[test]
    fn visibility_order_apply_persists_and_undo_keeps_ids() {
        let (mut e, mut v, a, _) = fixture();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let before = e.state().clone();
        let info = registry::info_for("osc.va").unwrap();
        let mut visible = rack::primary_set(&before.modules[&a], info);
        let i = info
            .params
            .iter()
            .position(|p| p.name == "waveform")
            .unwrap();
        visible[i] = false;
        let pulse = info.params.iter().position(|p| p.name == "pw").unwrap();
        visible[pulse] = true;
        let original = rack::layout(&before, &rack::View::default())
            .get(a)
            .unwrap()
            .ctl("base_hz")
            .unwrap()
            .geo;
        v.choose = Some((a, visible));
        let mut order = rack::control_order(&before.modules[&a], info);
        order.swap(0, pulse);
        v.face_order = Some(order.clone());
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        click(&ctx, &mut e, &mut v, "face:apply");
        assert!(!rack::primary_set(&e.state().modules[&a], info)[i]);
        assert_eq!(rack::control_order(&e.state().modules[&a], info), order);
        let placed = rack::layout(e.state(), &rack::View::default());
        let module = placed.get(a).unwrap();
        assert_ne!(module.ctl("base_hz").unwrap().geo, original);
        assert!(
            module.ctl("pw").unwrap().geo.label_pos().x
                < module.ctl("base_hz").unwrap().geo.label_pos().x
        );
        assert!(kabl_engine::runtime::runtime_changes(&before, e.state())
            .unwrap()
            .is_empty());
        let dir = tempfile::tempdir().unwrap();
        kabl_core::save(dir.path(), e.log()).unwrap();
        let loaded = kabl_core::load(dir.path()).unwrap();
        assert_eq!(loaded.state(), e.state());
        e.undo();
        assert_eq!(e.state(), &before);
    }
    #[test]
    fn real_composite_body_lifts_freely_drop_and_cancel_preserve_internals() {
        for authored in [false, true] {
            let (mut e, mut v, _, id) = fixture();
            if authored {
                let mut c = e.state().composites[&id].clone();
                c.panel = Some(Box::default());
                e.restore_to(vec![Op::SetComposite { id, value: Some(c) }]);
            }
            let ctx = egui::Context::default();
            for _ in 0..3 {
                frame(&ctx, &mut e, &mut v, vec![]);
            }
            v.zoom = 0.75;
            v.pan = vec2(30., 40.);
            frame(&ctx, &mut e, &mut v, vec![]);
            let before = e.state().clone();
            let pan = v.pan;
            let r = v.hits[&format!("composite:{id}:body")];
            let p = r.left_top() + vec2(13., 14.);
            frame(
                &ctx,
                &mut e,
                &mut v,
                vec![egui::Event::PointerMoved(p), button(p, true)],
            );
            let to = p + vec2(-80., 210.);
            for _ in 0..3 {
                frame(&ctx, &mut e, &mut v, vec![egui::Event::PointerMoved(to)]);
            }
            let mv = v.moving.as_ref().unwrap();
            assert_eq!(mv.item, Item::Composite(id));
            assert_eq!(v.selected_composite, Some(id));
            assert!((v.xf().p(pos2(mv.live.x, mv.live.y) + mv.grab) - to).length() < 0.001);
            assert_eq!(v.pan, pan);
            assert_eq!(e.state(), &before);
            assert!(v.hits.contains_key("rack:drop-preview"));
            frame(&ctx, &mut e, &mut v, vec![button(to, false)]);
            assert_ne!(e.state().composites[&id].pos, before.composites[&id].pos);
            assert_eq!(e.state().modules, before.modules);
            e.undo();
            assert_eq!(e.state(), &before);
        }
    }
    #[test]
    fn nested_back_restores_each_camera_selection_and_saved_leaf_positions() {
        let (mut e, mut v, a, parent) = fixture();
        let leaf = *e.state().composites[&parent].members.first().unwrap();
        let child = parent + 1;
        let mut nested = e.state().composites[&parent].clone();
        nested.parent = Some(parent);
        nested.name = "Nested".into();
        nested.definition.id = "nested-test".into();
        e.restore_to(vec![Op::SetComposite {
            id: child,
            value: Some(nested),
        }]);
        let mut c = e.state().composites[&child].clone();
        c.parent = Some(parent);
        let mut p = e.state().composites[&parent].clone();
        p.members.remove(&leaf);
        e.restore_to(vec![
            Op::SetComposite {
                id: parent,
                value: Some(p),
            },
            Op::SetComposite {
                id: child,
                value: Some(c),
            },
        ]);
        let before = e.state().clone();
        v.pan = vec2(40., 60.);
        v.zoom = 0.65;
        v.selected_module = Some(a);
        v.inspected = Some((leaf, "cutoff_hz".into()));
        v.last_inspected = v.inspected.clone();
        v.enter_composite(e.state(), parent);
        assert!(layout(e.state(), &v).mods.is_empty());
        assert!(layout(e.state(), &v).faces.contains_key(&child));
        v.pan = vec2(70., 90.);
        v.zoom = 0.8;
        v.selected_composite = Some(child);
        v.enter_composite(e.state(), child);
        assert_eq!(layout(e.state(), &v).mods.len(), 1);
        assert_eq!(layout(e.state(), &v).mods[0].id, leaf);
        v.back();
        assert_eq!(v.scope, Some(parent));
        assert_eq!((v.pan, v.zoom), (vec2(70., 90.), 0.8));
        assert_eq!(v.selected_composite, Some(child));
        v.back();
        assert_eq!(v.scope, None);
        assert_eq!((v.pan, v.zoom), (vec2(40., 60.), 0.65));
        assert_eq!(v.selected_module, Some(a));
        assert_eq!(v.last_inspected, v.inspected);
        assert_eq!(e.state(), &before);
    }
    #[test]
    fn edge_scroll_reaches_new_rows_without_editing_until_drop() {
        let (mut e, mut v, a, _) = fixture();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let r = layout(e.state(), &v).get(a).unwrap().face;
        let before = e.state().clone();
        let pan = v.pan;
        v.moving = Some(Moving {
            item: Item::Module(a),
            grab: vec2(15., 20.),
            live: Vec2 {
                x: r.left(),
                y: r.top(),
            },
            origin: r.min,
            width: r.width(),
            cancelled: false,
        });
        let at = pos2(v.canvas.center().x, v.canvas.bottom() - 2.);
        for _ in 0..100 {
            frame(&ctx, &mut e, &mut v, vec![egui::Event::PointerMoved(at)]);
        }
        assert!(v.pan.y < pan.y - 200.);
        assert_eq!(e.state(), &before);
        frame(&ctx, &mut e, &mut v, vec![button(at, false)]);
        assert!(rack::row_of(e.state().modules[&a].pos.y) >= 2);
        e.undo();
        assert_eq!(e.state(), &before);
    }
    #[test]
    fn add_appends_after_composites_and_preserves_existing_position() {
        let (mut e, mut v, a, id) = fixture();
        e.remove_module(a);
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut e, &mut v, vec![]);
        }
        let face = layout(e.state(), &v).faces[&id];
        let before = e.state().composites[&id].pos;
        click(&ctx, &mut e, &mut v, "add-menu");
        click(&ctx, &mut e, &mut v, "add");
        assert_eq!(layout(e.state(), &v).faces[&id], face);
        assert_eq!(e.state().composites[&id].pos, before);
        assert!(
            layout(e.state(), &v)
                .get(v.selected_module.unwrap())
                .unwrap()
                .face
                .left()
                >= face.right()
        );
    }
    #[test]
    fn light_and_dark_apply_to_drawers_dialogs_and_workspace() {
        let light = theme(&crate::style::builtin_a(false));
        let dark = theme(&crate::style::builtin_a(true));
        assert!(light.chrome.r() > 200 && light.rack.r() > 200 && light.ctext.r() < 80);
        assert!(dark.chrome.r() < 60 && dark.ctext.r() > 200);
        for t in [light, dark] {
            let v = t.visuals();
            assert_eq!(v.panel_fill, t.chrome);
            assert_eq!(v.window_fill, t.chrome);
            assert_eq!(v.extreme_bg_color, t.rack);
            assert_eq!(v.override_text_color, Some(t.ctext));
        }
    }
}
