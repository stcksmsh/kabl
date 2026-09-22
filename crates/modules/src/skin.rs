//! Optional per-module visual skin — not a brief-specified feature, an owner ask: "custom modules
//! can use their own images as their background and specify where to put their
//! jacks/ins/outs/switches/readouts". `ModuleInfo.skin` is `None` for every built-in except one
//! demo (`osc.va` — see decisions.md "Module skins: custom panel art") that proves the mechanism
//! end-to-end with a real (if placeholder) asset. `None` means `kabl-ui` falls back to its own
//! auto-layout procedural panel — every other built-in still gets that, unaffected by this file
//! existing.
//!
//! Deliberately UI-framework-agnostic: this crate has no `egui`/GPU dependency, so
//! `background_image` is raw embedded PNG bytes (`include_bytes!` at the point a skin constant is
//! defined), not a filesystem path or an `egui`-specific texture type — portable to any renderer,
//! and it never fails to find the file at runtime since it's compiled in.
//!
//! Only `Jack` and `Knob` are wired up in `kabl-ui`'s renderer today — the two kinds every one of
//! the 9 built-ins actually needs (ports and continuous params). `Switch`/`Readout` are declared
//! for a module that needs a discrete toggle or a live numeric display, but nothing renders them
//! yet: add that when a real module needs one, not speculatively now.

/// What a `ControlSkin` position marks and what it must match by `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// A cable connection point — `id` must match one of the module's `PortInfo.name`s.
    Jack,
    /// A continuous, draggable control — `id` must match one of the module's `ParamInfo.name`s.
    Knob,
    /// Declared, not yet rendered by `kabl-ui` — no built-in has a discrete on/off param to bind
    /// one to yet.
    Switch,
    /// Declared, not yet rendered by `kabl-ui` — no built-in has a live numeric readout yet.
    Readout,
}

#[derive(Debug, Clone, Copy)]
pub struct ControlSkin {
    pub id: &'static str,
    pub kind: ControlKind,
    /// Position within the panel, normalized to `ModuleSkin.panel_size` — `(0.0, 0.0)` is the
    /// panel's top-left corner, `(1.0, 1.0)` its bottom-right. Normalized (not pixels) so the
    /// same skin stays correct if `panel_size` ever needs to change without re-deriving every
    /// control's coordinates by hand.
    pub pos: (f32, f32),
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleSkin {
    /// Panel size in UI points. The background image (if any) is drawn stretched to exactly this
    /// size, so `controls`' normalized positions land where the image was actually designed for
    /// them.
    pub panel_size: (f32, f32),
    /// Raw PNG bytes, embedded via `include_bytes!` — `None` means the renderer draws its own
    /// procedural panel (dark faceplate + category accent strip) instead of an image.
    pub background_image: Option<&'static [u8]>,
    /// Every control this skin explicitly places. A port or param with no entry here simply isn't
    /// drawn by a skin-aware renderer — every control a module actually has should appear, or it
    /// becomes unreachable from that renderer's UI.
    pub controls: &'static [ControlSkin],
}
