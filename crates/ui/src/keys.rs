//! Notes without a MIDI controller: the computer keyboard (standalone) and the on-screen key
//! strip under the rack (both builds). Both feed one set of held notes; the host drains
//! `Keys::out` into its audio path as controller key events on channel 1, so a keyboard
//! patch hears them exactly as it would a MIDI keyboard (mono and legato included).
//!
//! A note sounds while anything holds it: two computer keys an octave apart, or a computer
//! key and the strip, start one voice and release it when the last of them lets go. A key
//! released after an octave change ends the note it started. Losing window focus, switching
//! scene and loading a sound end every note started here.

use crate::kit::{self, Tone};
use crate::style::{Role, Style};
use crate::UiState;
use egui::{Color32, Key, Rect, Sense, Ui};
use kabl_engine::keyboard::KeyEvent;

/// Every note from the computer keys and the strip has this velocity.
pub const VELOCITY: u8 = 100;
/// The note of the A key at octave 0 (middle C).
pub const BASE: u8 = 60;
/// Octave shift range, either way.
pub const MAX_OCTAVE: i8 = 3;
/// Notes the strip shows: two octaves and the top C.
pub const STRIP_NOTES: u8 = 25;
/// Height of the open strip.
pub const STRIP_H: f32 = 64.0;

/// The A row plays white keys from C, the W row the black keys between them.
const LAYOUT: [(Key, u8); 18] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
    (Key::Semicolon, 16),
    (Key::Quote, 17),
];

/// The scenes the toolbar switches between; a switch ends every note.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scene {
    Rack,
    Perform,
    Sounds,
}

fn scene(ui_state: &UiState) -> Scene {
    if ui_state.browser_open {
        Scene::Sounds
    } else if ui_state.perform_open {
        Scene::Perform
    } else {
        Scene::Rack
    }
}

pub struct Keys {
    /// Computer-keyboard notes (on in the standalone, off in the plugin, where the host owns
    /// the keyboard).
    pub computer: bool,
    /// The on-screen strip is shown.
    pub strip_open: bool,
    /// Octave shift of the computer keys and the strip, `-MAX_OCTAVE..=MAX_OCTAVE`.
    pub octave: i8,
    /// Key events for the host to send to the audio path, oldest first.
    pub out: Vec<KeyEvent>,
    /// Computer keys held, each with the note it started.
    held: Vec<(Key, u8)>,
    /// The note the strip pointer holds.
    strip: Option<u8>,
    /// How many things (computer keys, the strip) hold each note.
    holders: [u8; 128],
    scene: Option<Scene>,
    /// `UiState::loaded` last frame: the host clears it once the new sound is installed.
    loaded: bool,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            computer: false,
            strip_open: false,
            octave: 0,
            out: Vec::new(),
            held: Vec::new(),
            strip: None,
            holders: [0; 128],
            scene: None,
            loaded: false,
        }
    }
}

impl Keys {
    /// The note of semitone `s` above the current octave's C, if it is a MIDI note.
    fn note(&self, s: u8) -> Option<u8> {
        u8::try_from(BASE as i32 + 12 * self.octave as i32 + s as i32)
            .ok()
            .filter(|&n| n < 128)
    }

    fn hold(&mut self, note: u8) {
        let h = &mut self.holders[note as usize];
        *h = h.saturating_add(1);
        if *h == 1 {
            self.out.push(KeyEvent::On {
                note,
                velocity: VELOCITY,
            });
        }
    }

    fn release(&mut self, note: u8) {
        let h = &mut self.holders[note as usize];
        if *h == 0 {
            return;
        }
        *h -= 1;
        if *h == 0 {
            self.out.push(KeyEvent::Off { note });
        }
    }

    /// Whether `note` sounds from here.
    pub fn sounding(&self, note: u8) -> bool {
        self.holders.get(note as usize).is_some_and(|&h| h > 0)
    }

    /// Whether any note sounds from here.
    pub fn any(&self) -> bool {
        self.holders.iter().any(|&h| h > 0)
    }

    /// Ends every note started here (one note-off each) and forgets what was held.
    pub fn all_off(&mut self) {
        for note in 0..128u8 {
            if self.holders[note as usize] > 0 {
                self.out.push(KeyEvent::Off { note });
            }
        }
        self.holders = [0; 128];
        self.held.clear();
        self.strip = None;
    }

    pub fn shift_octave(&mut self, by: i8) {
        self.octave = (self.octave + by).clamp(-MAX_OCTAVE, MAX_OCTAVE);
    }

    /// A computer key went down or up. Returns whether it was one of ours (for a release:
    /// one that was held).
    pub fn key(&mut self, key: Key, pressed: bool) -> bool {
        if !pressed {
            let Some(i) = self.held.iter().position(|&(k, _)| k == key) else {
                return false;
            };
            let (_, note) = self.held.swap_remove(i);
            self.release(note);
            return true;
        }
        match key {
            Key::Z => self.shift_octave(-1),
            Key::X => self.shift_octave(1),
            _ => {
                let Some(&(_, s)) = LAYOUT.iter().find(|&&(k, _)| k == key) else {
                    return false;
                };
                // Already down: an auto-repeat the OS did not mark. No retrigger.
                if self.held.iter().any(|&(k, _)| k == key) {
                    return true;
                }
                if let Some(note) = self.note(s) {
                    self.held.push((key, note));
                    self.hold(note);
                }
            }
        }
        true
    }

    /// The strip pointer is on `note` (`None`: let go).
    pub fn strip_to(&mut self, note: Option<u8>) {
        if self.strip == note {
            return;
        }
        if let Some(old) = self.strip.take() {
            self.release(old);
        }
        if let Some(n) = note.filter(|&n| n < 128) {
            self.strip = Some(n);
            self.hold(n);
        }
    }

    /// The lowest note the strip shows.
    pub fn strip_low(&self) -> u8 {
        self.note(0).unwrap_or(0).min(127 - (STRIP_NOTES - 1))
    }
}

/// Computer keys for this frame, before any widget reads the keyboard: their events are
/// taken out of the input so a key that plays never also works as a shortcut. Presses count
/// only when nothing else wants the keyboard (a text field, a popup, a dialog: the guard the
/// tour uses); releases always count, so a note never hangs on a field that took focus. A
/// window without focus ends every note.
pub fn input(ui_state: &mut UiState, ctx: &egui::Context) {
    if !ctx.input(|i| i.focused) {
        if ui_state.keys.any() {
            ui_state.keys.all_off();
        }
        return;
    }
    let free = ui_state.keys.computer
        && ui_state.browser.dialog.is_none()
        && !crate::routing::wants_text_input(ctx)
        && !egui::Popup::is_any_open(ctx);
    let keys = &mut ui_state.keys;
    ctx.input_mut(|i| {
        i.events.retain(|e| {
            let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } = *e
            else {
                return true;
            };
            // Where the key is, not what it types: the piano rows stay put on any layout.
            let key = physical_key.unwrap_or(key);
            if !pressed {
                // A release of a key we hold ends its note, whatever has focus now.
                let ours = keys.held.iter().any(|&(k, _)| k == key);
                if ours {
                    keys.key(key, false);
                }
                return !ours;
            }
            if !free || !modifiers.is_none() {
                return true;
            }
            if repeat {
                return !(matches!(key, Key::Z | Key::X) || LAYOUT.iter().any(|&(k, _)| k == key));
            }
            !keys.key(key, true)
        });
    });
}

/// After the frame: a scene switch or a sound load ends every note.
pub fn watch(ui_state: &mut UiState) {
    let now = scene(ui_state);
    let switched = ui_state.keys.scene.replace(now).is_some_and(|s| s != now);
    let loaded = ui_state.loaded && !std::mem::replace(&mut ui_state.keys.loaded, ui_state.loaded);
    if (switched || loaded) && ui_state.keys.any() {
        ui_state.keys.all_off();
    }
}

/// Which keys kabl uses, as of the end of a frame. A plugin window asks `keeps` as each key
/// arrives, before kabl's next frame runs, so a key kabl uses stays with kabl and any other goes
/// only to the host (Space to its transport, its shortcuts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Owner {
    /// A text field has focus: every key is kabl's.
    text: bool,
    /// Another widget (a knob, a slider) has focus: it takes the arrows.
    focused: bool,
    /// The computer keys play notes (`input`'s `free`).
    notes: bool,
    /// A opens the add-module menu (computer keys off).
    add_menu: bool,
    /// The browser list takes Up and Down.
    browser_list: bool,
    /// A browser row is selected: Space previews it, Enter loads it.
    browser_row: bool,
    /// The design page is open: it takes the arrows, Delete and its own undo.
    design: bool,
    /// The rack's undo and redo are live.
    undo: bool,
    /// A menu is open: Delete removes the module of its module menu.
    popup: bool,
    /// No browser dialog is open: Ctrl+S saves.
    save: bool,
}

impl Owner {
    pub fn of(ui_state: &UiState, ctx: &egui::Context) -> Self {
        let text = crate::routing::wants_text_input(ctx);
        let dialog = ui_state.browser.dialog.is_some();
        let popup = egui::Popup::is_any_open(ctx);
        let focused = ctx.memory(|m| m.focused().is_some());
        let browser_list = ui_state.browser_open && !dialog && !focused;
        Owner {
            text,
            focused,
            notes: ui_state.keys.computer && !dialog && !text && !popup,
            add_menu: !ui_state.keys.computer && !text,
            browser_list,
            browser_row: browser_list && ui_state.browser.selected.is_some(),
            design: ui_state.panels.page.is_some(),
            undo: !dialog && ui_state.moving.is_none(),
            popup,
            save: !dialog,
        }
    }

    /// Whether kabl uses `key` (`None`: a key egui has no name for) with `modifiers` held.
    pub fn keeps(&self, key: Option<Key>, modifiers: egui::Modifiers) -> bool {
        if self.text {
            return true;
        }
        let Some(key) = key else {
            return false;
        };
        let plain = modifiers.is_none();
        let arrow = matches!(
            key,
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
        );
        if arrow && (plain || modifiers == egui::Modifiers::SHIFT) && self.focused {
            return true;
        }
        if modifiers.command && !modifiers.alt {
            return match key {
                Key::Z => self.undo || self.design,
                Key::Y => !modifiers.shift && (self.undo || self.design),
                Key::S => !modifiers.shift && self.save,
                _ => false,
            };
        }
        if !plain {
            return false;
        }
        let note = matches!(key, Key::Z | Key::X) || LAYOUT.iter().any(|&(k, _)| k == key);
        match key {
            _ if note && self.notes => true,
            Key::A => self.add_menu,
            // Dialogs, menus, toasts, drags, lessons and the dock all close or cancel on it.
            Key::Escape => true,
            Key::Space => self.browser_row,
            Key::Enter => self.browser_row,
            Key::ArrowUp | Key::ArrowDown => self.browser_list || self.design,
            Key::ArrowLeft | Key::ArrowRight => self.design,
            Key::Delete => self.design || self.popup,
            _ => false,
        }
    }
}

fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[note as usize % 12], note as i32 / 12 - 1)
}

/// The key rects of the strip in `area`: white keys first, then the black keys on top.
fn layout(low: u8, area: Rect) -> Vec<(u8, Rect)> {
    let notes = low..low + STRIP_NOTES;
    let whites = notes.clone().filter(|&n| !is_black(n)).count().max(1);
    let w = area.width() / whites as f32;
    let mut out = Vec::with_capacity(STRIP_NOTES as usize);
    let mut x = area.left();
    let mut blacks = Vec::new();
    for n in notes {
        if is_black(n) {
            let bw = w * 0.6;
            blacks.push((
                n,
                Rect::from_min_size(
                    egui::pos2(x - bw / 2.0, area.top()),
                    egui::vec2(bw, area.height() * 0.6),
                ),
            ));
        } else {
            out.push((
                n,
                Rect::from_min_size(egui::pos2(x, area.top()), egui::vec2(w, area.height())),
            ));
            x += w;
        }
    }
    out.extend(blacks);
    out
}

/// The note under `pos`: black keys are on top.
fn hit(keys: &[(u8, Rect)], pos: egui::Pos2) -> Option<u8> {
    keys.iter()
        .rev()
        .find(|(_, r)| r.contains(pos))
        .map(|k| k.0)
}

/// The open strip: octave buttons, then the keys. Press and drag to play.
fn strip(ui_state: &mut UiState, ui: &mut Ui, st: &Style) {
    let full = ui.max_rect();
    let side = 150.0;
    let controls = Rect::from_min_size(full.min, egui::vec2(side, full.height()));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(controls.shrink(st.sp(1)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.horizontal(|ui| {
                let down = ui.add(kit::Button::new(st, "−").small().ghost());
                ui_state.record("keys:octave-down".into(), down.rect);
                if down.clicked() {
                    ui_state.keys.shift_octave(-1);
                }
                let low = ui_state.keys.strip_low();
                kit::label(ui, st, Role::Value, Tone::Text2, note_name(low));
                let up = ui.add(kit::Button::new(st, "+").small().ghost());
                ui_state.record("keys:octave-up".into(), up.rect);
                if up.clicked() {
                    ui_state.keys.shift_octave(1);
                }
            });
            let hint = if ui_state.keys.computer {
                "Keys A–K · Z X octave"
            } else {
                "Click to play"
            };
            kit::label(ui, st, Role::Caption, Tone::Text3, hint);
        },
    );
    let area = Rect::from_min_max(
        egui::pos2(full.left() + side, full.top() + st.sp(1)),
        egui::pos2(full.right() - st.sp(2), full.bottom() - st.sp(1)),
    );
    ui_state.record("keys:strip".into(), area);
    let low = ui_state.keys.strip_low();
    let keys = layout(low, area);
    let resp = ui.interact(area, egui::Id::new("keys:strip"), Sense::click_and_drag());
    let down = ui.input(|i| i.pointer.primary_down());
    let pos = ui.input(|i| i.pointer.interact_pos());
    // Starts with a press on the strip; a drag slides from key to key; letting go ends it.
    // An unfocused window has ended every note (`input`); a button still down must not start
    // them again each frame.
    let focused = ui.input(|i| i.focused);
    let playing =
        focused && down && (resp.is_pointer_button_down_on() || ui_state.keys.strip.is_some());
    let note = playing.then(|| pos.and_then(|p| hit(&keys, p))).flatten();
    if playing || ui_state.keys.strip.is_some() {
        ui_state.keys.strip_to(note);
    }
    let p = ui.painter_at(area);
    for (n, r) in &keys {
        let on = ui_state.keys.sounding(*n);
        let fill = match (is_black(*n), on) {
            (_, true) => st.roles.accent,
            (true, false) => st.roles.text2,
            (false, false) => st.roles.raised,
        };
        p.rect_filled(r.shrink(1.0), 3.0, fill);
        if !is_black(*n) {
            p.rect_stroke(
                r.shrink(1.0),
                3.0,
                egui::Stroke::new(1.0, st.roles.line),
                egui::StrokeKind::Inside,
            );
            if n % 12 == 0 {
                let g = p.layout_no_wrap(
                    note_name(*n),
                    st.font(Role::Caption),
                    if on {
                        st.roles.on_accent
                    } else {
                        st.roles.text3
                    },
                );
                p.galley(
                    egui::pos2(
                        r.center().x - g.size().x / 2.0,
                        r.bottom() - g.size().y - 3.0,
                    ),
                    g,
                    Color32::PLACEHOLDER,
                );
            }
        }
    }
    if ui_state.keys.any() {
        ui.ctx().request_repaint();
    }
}

/// The status bar's "Keys" button: shows or hides the strip.
pub fn toggle(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let r = ui
        .add(
            kit::Button::new(st, "Keys")
                .ghost()
                .small()
                .selected(ui_state.keys.strip_open),
        )
        .on_hover_text("Play notes without a MIDI controller: an on-screen key strip");
    ui_state.record("keys:toggle".into(), r.rect);
    if r.clicked() {
        ui_state.keys.strip_open = !ui_state.keys.strip_open;
    }
}

/// The strip's panel under the rack and the Perform view; Sounds has its own audition.
pub fn panel(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    if ui_state.keys.strip_open && !ui_state.browser_open {
        egui::Panel::bottom("kabl-keys")
            .exact_size(STRIP_H)
            .resizable(false)
            .frame(kit::panel_frame(st))
            .show(ui, |ui| strip(ui_state, ui, st));
    }
}
