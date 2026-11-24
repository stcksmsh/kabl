//! L4: turning a macro on the Perform page reveals what it changes. The card's list expands to
//! module numbers and signed depths (`explain::destinations`, sorted by depth), the target
//! controls' rings pulse in the rack while it moves, and both settle after the hold.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use egui::{Event, Pos2, RawInput, Rect};
use kabl_core::{ModuleId, PortRef};
use kabl_ui::explain::{self, Reach, REVEAL_HOLD, REVEAL_SHOWN};
use kabl_ui::{composites, cues, show, MacroLive, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    events: Vec<Event>,
    t: f64,
}

impl H {
    fn new(patch: &str) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../patches")
            .join(patch);
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            events: Vec::new(),
            t: 0.0,
        };
        h.ui.perform_open = true;
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let size = egui::vec2(1440.0, 900.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let _shapes = self.ctx.end_pass();
    }

    /// Drags the Perform knob of macro `port` up by real pointer input.
    fn turn(&mut self, id: ModuleId, port: &str) {
        let key = format!("pslider:{id}.{port}");
        let p = self.ui.hits.get(&key).expect("macro knob").center();
        self.events.push(Event::PointerMoved(p));
        self.frame();
        self.events.push(Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        self.frame();
        let mut q = p;
        for _ in 0..6 {
            q.y -= 8.0;
            self.events.push(Event::PointerMoved(q));
            self.frame();
        }
        self.events.push(Event::PointerButton {
            pos: q,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        self.frame();
        self.frame();
    }

    /// Lets the hold run out.
    fn settle(&mut self) {
        self.t += REVEAL_HOLD + 0.1;
        self.frame();
        self.frame();
    }

    fn macro_module(&self) -> ModuleId {
        let s = self.editor.state();
        *s.modules.iter().find(|(_, m)| m.kind == "macro").unwrap().0
    }
}

/// The non-bypassed parameter destinations: what must pulse.
fn live_targets(h: &H, id: ModuleId, port: &str) -> BTreeSet<(ModuleId, String)> {
    let (rows, _) = explain::destinations(h.editor.state(), id, port);
    rows.iter()
        .filter_map(|d| match &d.reach {
            Reach::Param {
                param,
                bypass: false,
                ..
            } => Some((d.to, param.name.to_string())),
            _ => None,
        })
        .collect()
}

fn pulsing(h: &H) -> BTreeSet<(ModuleId, String)> {
    h.ui.macro_pulse
        .iter()
        .map(|&(id, p)| (id, p.to_string()))
        .collect()
}

/// The macro output that drives two modules of the same kind (Laundromat: three ladders).
fn twin_port(h: &H, id: ModuleId) -> &'static str {
    let s = h.editor.state();
    kabl_modules::builtins::MACRO_INFO
        .params
        .iter()
        .map(|p| p.name)
        .find(|port| {
            let (rows, _) = explain::destinations(s, id, port);
            rows.iter().enumerate().any(|(i, a)| {
                rows[i + 1..].iter().any(|b| {
                    matches!(a.reach, Reach::Param { .. })
                        && matches!(b.reach, Reach::Param { .. })
                        && a.to != b.to
                        && s.modules[&a.to].kind == s.modules[&b.to].kind
                })
            })
        })
        .expect("a macro on two same-kind modules")
}

#[test]
fn turning_a_macro_names_each_target_by_number_with_its_signed_depth() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    let key = format!("{id}.{port}");
    assert!(!h.ui.hits.contains_key(&format!("preveal:{key}")));
    h.turn(id, port);
    assert!(
        h.ui.hits.contains_key(&format!("preveal:{key}")),
        "the card expands while the macro moves"
    );
    let k = port.as_bytes()[1] - b'1';
    assert!(h.ui.macro_turning(id, k, h.t));
    let lines = explain::reveal(h.editor.state(), &h.ui, id, port);
    let (rows, _) = explain::destinations(h.editor.state(), id, port);
    for d in &rows {
        let Reach::Param { amount, .. } = d.reach else {
            continue;
        };
        let num = format!("#{} ", d.to);
        let depth = format!("{:+.0} %", amount * 100.0);
        assert!(
            lines
                .iter()
                .any(|l| l.text.contains(&num) && l.text.contains(&depth) && l.depth == amount),
            "no line with {num}and {depth}: {lines:?}"
        );
    }
    // Two modules of one kind read apart by their numbers.
    let nums: BTreeSet<&str> = lines
        .iter()
        .filter_map(|l| l.text.split(' ').nth(1))
        .collect();
    assert!(nums.len() >= 2, "{lines:?}");
    h.settle();
    assert!(!h.ui.hits.contains_key(&format!("preveal:{key}")));
}

#[test]
fn the_target_rings_pulse_while_the_macro_moves_and_stop_after_the_hold() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    assert!(
        h.ui.macro_pulse.is_empty(),
        "opening a sound pulses nothing"
    );
    h.turn(id, port);
    let want = live_targets(&h, id, port);
    assert!(!want.is_empty());
    assert_eq!(pulsing(&h), want);
    h.settle();
    assert!(h.ui.macro_pulse.is_empty());
}

#[test]
fn a_bypassed_route_is_dimmed_and_does_not_pulse() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    let (rows, _) = explain::destinations(h.editor.state(), id, port);
    let off = rows
        .iter()
        .find(|d| matches!(d.reach, Reach::Param { .. }))
        .unwrap();
    let (off_cable, off_to) = (off.cable, off.to);
    let Reach::Param { param, .. } = off.reach else {
        unreachable!()
    };
    h.editor.set_route_bypass(off_cable, true);
    h.frame();
    h.turn(id, port);
    let lines = explain::reveal(h.editor.state(), &h.ui, id, port);
    let line = lines.iter().find(|l| l.bypass).expect("bypassed line");
    assert_eq!(lines.iter().filter(|l| l.bypass).count(), 1);
    assert!(line.text.contains(&format!("#{off_to} ")), "{line:?}");
    assert!(line.text.ends_with("(bypassed)"), "{line:?}");
    assert_eq!(line.pulse, None);
    let rows = explain::reveal_rows(&lines);
    assert!(rows.iter().any(|(t, dim)| *dim && t == &line.text));
    assert!(!pulsing(&h).contains(&(off_to, param.name.to_string())));
    assert_eq!(pulsing(&h), live_targets(&h, id, port));
}

#[test]
fn many_destinations_show_the_deepest_first_then_how_many_more() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    // A spare macro output, routed to six controls of one module at different depths.
    let port = "m8";
    assert!(explain::destinations(h.editor.state(), id, port)
        .0
        .is_empty());
    let target = *h
        .editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| {
            m.kind != "macro"
                && kabl_modules::registry::info_for(&m.kind).is_some_and(|i| i.params.len() >= 6)
        })
        .unwrap()
        .0;
    let kind = h.editor.state().modules[&target].kind.clone();
    let params: Vec<&str> = kabl_modules::registry::info_for(&kind)
        .unwrap()
        .params
        .iter()
        .map(|p| p.name)
        .take(6)
        .collect();
    assert_eq!(params.len(), 6, "{kind} has six params");
    let depths = [0.1, -0.6, 0.3, 0.9, -0.2, 0.5];
    for (p, d) in params.iter().zip(depths) {
        let from = PortRef::Module {
            id,
            port: port.into(),
        };
        let c = h.editor.connect_route(from, target, p);
        h.editor.set_route_amount(c, d, true);
    }
    h.frame();
    let lines = explain::reveal(h.editor.state(), &h.ui, id, port);
    let got: Vec<f32> = lines.iter().map(|l| l.depth).collect();
    assert_eq!(got, vec![0.9, -0.6, 0.5, 0.3, -0.2, 0.1]);
    assert!(lines[1].text.ends_with("-60 %"), "{:?}", lines[1]);
    assert!(lines[0].text.ends_with("+90 %"), "{:?}", lines[0]);
    let rows = explain::reveal_rows(&lines);
    assert_eq!(rows.len(), REVEAL_SHOWN + 1);
    assert_eq!(rows[REVEAL_SHOWN].0, format!("+{} more", 6 - REVEAL_SHOWN));
    for (row, line) in rows[..REVEAL_SHOWN].iter().zip(&lines) {
        assert_eq!(row.0, line.text);
    }
}

#[test]
fn a_cue_morphing_a_macro_reveals_nothing_during_or_after_the_morph() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    let k = port.as_bytes()[1] - b'1';
    let cues_id = *h
        .editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "cues")
        .expect("Laundromat has cues")
        .0;
    let n = cues::cues(h.editor.state(), cues_id)[0].n;
    cues::set_macro(&mut h.editor, cues_id, n, id, k, Some(0.9));
    cues::set_morph(&mut h.editor, cues_id, n, 2.0);
    cues::launch(&mut h.ui.launches, h.editor.state(), cues_id, n).unwrap();
    h.frame();
    // The audio thread reports the morph as it goes, as `app::frame` feeds it in.
    for i in 0..20 {
        let progress = i as f32 / 20.0;
        h.ui.macro_live.insert(
            (id, k),
            (
                MacroLive {
                    value: 0.2 + 0.7 * progress,
                    to: 0.9,
                    progress,
                },
                Instant::now(),
            ),
        );
        h.frame();
        assert!(!h.ui.hits.contains_key(&format!("preveal:{id}.{port}")));
        assert!(h.ui.macro_pulse.is_empty());
    }
    // The reports stop: after 400 ms the knob reads its stored value again, quietly.
    let (last, _) = h.ui.macro_live[&(id, k)];
    h.ui.macro_live
        .insert((id, k), (last, Instant::now() - Duration::from_millis(500)));
    assert!(h.ui.live_macro(id, k).is_none());
    h.frame();
    h.frame();
    assert!(!h.ui.hits.contains_key(&format!("preveal:{id}.{port}")));
    assert!(h.ui.macro_pulse.is_empty());
    assert!(!h.ui.macro_turning(id, k, h.t));
}

#[test]
fn a_target_in_a_closed_composite_is_named_by_its_exposed_control_or_the_composite() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    let (rows, _) = explain::destinations(h.editor.state(), id, port);
    let (to, param, amount) = rows
        .iter()
        .find_map(|d| match d.reach {
            Reach::Param { param, amount, .. } if d.via.is_none() => {
                Some((d.to, param.name, amount))
            }
            _ => None,
        })
        .unwrap();
    let line_for = |h: &H| {
        explain::reveal(h.editor.state(), &h.ui, id, port)
            .into_iter()
            .find(|l| l.pulse == Some((to, param)))
            .expect("a line for the target")
    };
    let open = line_for(&h);
    let signed = format!(" {:+.0} %", amount * 100.0);
    let plain = open.text.strip_suffix(&signed).expect("ends in its depth");
    assert!(plain.contains(&format!("#{to} ")), "{plain}");

    let comp = composites::encapsulate(&mut h.editor, [to].into(), Default::default(), "Comp")
        .expect("encapsulate");
    h.ui.composites.open.remove(&comp);
    h.frame();
    assert_eq!(line_for(&h).text, format!("Comp ({plain}){signed}"));

    let target = PortRef::Param {
        id: to,
        param: param.into(),
    };
    composites::add_exposure(&mut h.editor, comp, target.clone(), true).expect("expose");
    let label = h.editor.state().composites[&comp]
        .controls
        .values()
        .find(|e| e.target == target)
        .unwrap()
        .label
        .clone();
    assert_eq!(line_for(&h).text, format!("Comp {label}{signed}"));

    // Opened, the module shows in the rack again and reads by its own number.
    h.ui.composites.open.insert(comp);
    assert_eq!(line_for(&h).text, open.text);
}

#[test]
fn a_macro_moved_by_midi_reveals_too() {
    let mut h = H::new("presets/laundromat");
    let id = h.macro_module();
    let port = twin_port(&h, id);
    let target = kabl_core::ParamTarget::Module {
        id,
        param: port.into(),
    };
    h.editor.set_param_cc(target, 0.9, false);
    h.frame();
    h.frame();
    assert!(h.ui.hits.contains_key(&format!("preveal:{id}.{port}")));
    assert_eq!(pulsing(&h), live_targets(&h, id, port));
}
