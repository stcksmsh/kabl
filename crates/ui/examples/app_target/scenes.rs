//! Whole-screen scenes. Each draws one complete 1440x900 / 1280x800 frame.
use crate::chrome::*;
use crate::prim::*;
use crate::rack::*;
use crate::tokens::Dir;
use egui::{pos2, vec2, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Rack,
    Perform,
    Composition,
    Faces,
    Wavetable,
    StepCable,
    Components,
}

impl Scene {
    pub const ALL: [Scene; 7] = [Scene::Rack, Scene::Perform, Scene::Composition, Scene::Faces, Scene::Wavetable, Scene::StepCable, Scene::Components];
    pub fn slug(self) -> &'static str {
        match self {
            Scene::Rack => "rack-sounds",
            Scene::Perform => "perform",
            Scene::Composition => "composition",
            Scene::Faces => "faces",
            Scene::Wavetable => "wavetable",
            Scene::StepCable => "step-cable",
            Scene::Components => "components",
        }
    }
    pub fn parse(s: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|x| x.slug() == s || x.slug().starts_with(s))
    }
}

pub struct Ctxs {
    pub reference: Rack,
    pub comp: Rack,
}

impl Ctxs {
    pub fn new() -> Self {
        Ctxs { reference: Rack::new(load("reference")), comp: Rack::new(load("composition")) }
    }
}

pub fn draw(cx: &Cx, d: &Ctxs, size: egui::Vec2, scene: Scene) {
    let k = cx.k;
    cx.p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, k.bg);
    match scene {
        Scene::Rack => rack_scene(cx, d, size),
        Scene::Perform => crate::perform::perform(cx, size),
        Scene::Composition => {
            let ids = [6u32, 7, 10, 36];
            let f = if cx.t >= 50.0 { ids[((cx.t - 50.0) / 0.9) as usize % 4] } else { 6 };
            crate::composition::composition(cx, d, size, f)
        }
        Scene::Faces => crate::closeup::faces(cx, d, size),
        Scene::Wavetable => crate::closeup::wavetable(cx, d, size),
        Scene::StepCable => crate::closeup::step_cable(cx, d, size),
        Scene::Components => crate::closeup::components(cx, size),
        _ => {}
    }
}

fn rack_bg(cx: &Cx, r: Rect) {
    let k = cx.k;
    cx.p.rect_filled(r, 0.0, k.rack);
    if k.dir == Dir::B {
        let mut x = r.left();
        while x < r.right() {
            cx.line(pos2(x, r.top()), pos2(x, r.bottom()), 1.0, a(k.line, 40));
            x += 30.0;
        }
    }
}

fn rack_scene(cx: &Cx, d: &Ctxs, size: egui::Vec2) {
    let k = cx.k;
    let ar = areas(k.dir, size, true, 44.0);
    let rk = &d.reference;
    rack_bg(cx, ar.center);
    let world = rk.world().expand2(vec2(14.0, 20.0));
    let xf = fit(world, ar.center, vec2(10.0, 6.0), 1.0);
    rk.rails(cx, xf, world.left(), world.right());
    rk.draw(cx, xf, Some(3), true, 1.0, true);
    toolbar(cx, ar.toolbar, "Rack", "Evolving Pad", true, &format!("{:.0}%", xf.s * 100.0), (true, false), 1);
    browser(cx, ar.left.unwrap(), 2, 4, "");
    rail(cx, ar.right, 1);
    status(cx, ar.status, (0.62 + 0.1 * (cx.t * 3.0).sin(), 0.55 + 0.1 * (cx.t * 2.7).sin()));
}

/// Recording script: (scene, time) per frame. Composition runs on t >= 50 so focus cycles.
pub fn timeline(fps: f32) -> Vec<(Scene, f32)> {
    let mut v = vec![];
    for (scene, secs, t0) in [(Scene::Rack, 3.5, 0.0), (Scene::Perform, 3.5, 1.2), (Scene::Composition, 3.6, 50.0), (Scene::Faces, 2.5, 0.0), (Scene::Wavetable, 2.5, 0.0), (Scene::StepCable, 3.0, 0.0)] {
        for i in 0..(secs * fps) as usize {
            v.push((scene, t0 + i as f32 / fps));
        }
    }
    v
}
