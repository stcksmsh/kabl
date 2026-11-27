use super::*;
use nice_plug::editor::dpi::PhysicalSize;
use nice_plug_egui::baseview::dpi::PhysicalSize as WindowPhysicalSize;
use nice_plug_egui::win32_dpi;

#[test]
fn windows_scale_follows_the_window_dpi() {
    for (dpi, scale) in [(96, 1.0), (144, 1.5), (192, 2.0)] {
        assert_eq!(win32_dpi::scale_factor(dpi), scale, "{dpi} DPI");
        // The window's DPI wins over the scale baseview last reported.
        assert_eq!(
            win32_dpi::effective_scale(Some(dpi), 1.25),
            scale,
            "{dpi} DPI"
        );
    }
    // Without per-monitor DPI (or for an invalid window) the reported scale stays.
    assert_eq!(win32_dpi::effective_scale(None, 1.25), 1.25);
    assert_eq!(win32_dpi::effective_scale(Some(0), 1.25), 1.25);

    // The window moves from a 150 % monitor to a 200 % one: the scale reacts, and the physical
    // size grows so the logical size stays.
    let at_150 = WindowPhysicalSize::new(1770, 1020);
    let from = win32_dpi::effective_scale(Some(144), 1.0);
    let to = win32_dpi::effective_scale(Some(192), from);
    assert_eq!(to, 2.0);
    assert_eq!(
        win32_dpi::rescaled(at_150, from, to),
        Some(WindowPhysicalSize::new(2360, 1360))
    );
    assert_eq!(
        win32_dpi::rescaled(WindowPhysicalSize::new(1180, 680), 1.0, 1.5),
        Some(at_150)
    );
    // An unchanged DPI leaves the window alone.
    assert_eq!(win32_dpi::rescaled(at_150, 1.5, 1.5), None);
}

#[test]
fn host_resize_is_constrained_to_the_fixed_editor_size() {
    let mut plugin = Instrument::default();
    let editor = plugin
        .editor(AsyncExecutor::new(Arc::new(|_| {}), Arc::new(|_| {})))
        .expect("kabl has an editor");
    let default = editor.size();
    assert_eq!(default, PhysicalSize::new(1180, 680));
    let hint = editor.resize_hint();
    assert!(!hint.can_resize);
    // The host asks for 1.5 times the default: the editor keeps its one size.
    let asked = PhysicalSize::new(1770, 1020);
    assert_eq!(hint.adjust_size(asked, default, 1.0), default);
    assert_eq!(editor.size(), default);
}

#[test]
fn host_is_asked_once_per_scale_change_and_never_again_after_a_refusal() {
    // First open at 100 %: the host already sized its frame for this.
    let mut frame = win32_dpi::HostFrame::default();
    assert!(!frame.should_ask(1.0));
    assert!(!frame.should_ask(1.0));

    // First open at 150 %: asked on the first frame only, whatever the host answers.
    let mut frame = win32_dpi::HostFrame::default();
    assert!(frame.should_ask(1.5));
    for _ in 0..10 {
        assert!(!frame.should_ask(1.5), "a refused size is not asked again");
    }

    // Moves to 200 % and back: one request per change.
    assert!(frame.should_ask(2.0));
    assert!(!frame.should_ask(2.0));
    assert!(frame.should_ask(1.0));
    assert!(!frame.should_ask(1.0));
}

#[test]
fn kabl_keeps_keys_only_while_a_text_field_has_focus() {
    use nice_plug_egui::baseview::EventStatus;
    use nice_plug_egui::{Key, KeyCapture};

    let space = Key::Character(" ".into());
    let note = Key::Character("a".into());
    // kabl leaves the capture at its default: every key reaches kabl, and the host also gets the
    // ones typed outside a text field.
    let capture = KeyCapture::default();
    assert_eq!(capture.status(&space, true), EventStatus::Captured);
    assert_eq!(capture.status(&note, true), EventStatus::Captured);
    assert_eq!(capture.status(&space, false), EventStatus::Ignored);
    assert_eq!(capture.status(&note, false), EventStatus::Ignored);
}

#[test]
fn a_press_goes_one_way_and_its_repeats_and_key_up_follow_it() {
    use nice_plug_egui::baseview::key_route::{KeyMessage::*, KeyRoute};

    const SPACE: u8 = 0x20;
    const A: u8 = 0x41;
    let host = || true;
    let kabl = || false;
    let unasked = || -> bool { panic!("only a new press is decided") };
    let mut route = KeyRoute::default();

    // Space goes to the host; a text field gains focus while it is held (kabl would now keep a
    // new press): the repeats and the key-up still go to the host.
    assert!(route.to_host(Down, SPACE, host));
    assert!(route.to_host(Repeat, SPACE, unasked));
    assert!(route.to_host(Repeat, SPACE, kabl));
    assert!(route.to_host(Up, SPACE, kabl));
    // The reverse: A typed into the field, which then loses focus. All of it stays with kabl.
    assert!(!route.to_host(Down, A, kabl));
    assert!(!route.to_host(Repeat, A, host));
    assert!(!route.to_host(Up, A, host));
    // A key-up whose key-down was never seen stays with kabl, and so do characters.
    assert!(!route.to_host(Up, SPACE, host));
    assert!(!route.to_host(Char, A, host));
    // A repeat of a press held before the window had it is a new press.
    assert!(route.to_host(Repeat, SPACE, host));
    assert!(route.to_host(Up, SPACE, kabl));

    // The window loses focus while Space (host) is held: a later unmatched key-up is not
    // forwarded to the host.
    assert!(route.to_host(Down, SPACE, host));
    route.clear();
    assert!(!route.to_host(Up, SPACE, host));
}

/// The plugin window's key path, as on Windows: kabl's `Owner` (from its last frame) decides
/// each new press in egui-baseview (`key_to_host`), which only hands egui the keys kabl keeps
/// and tells the keyboard hook the others are ignored; the hook's `KeyRoute` then retargets
/// exactly those to the host. Key-ups and repeats follow their key-down on both levels.
#[test]
fn kabl_keys_reach_only_kabl_and_host_keys_only_the_host() {
    use kabl_ui::keys::Owner;
    use kabl_ui::UiState;
    use nice_plug_egui::baseview::key_route::{KeyMessage, KeyRoute};
    use nice_plug_egui::baseview::EventStatus;
    use nice_plug_egui::key_to_host;
    use nice_plug_egui::keyboard_types::{Code, Key, KeyState, KeyboardEvent};

    #[derive(Default)]
    struct Window {
        app: KeyRoute<Code>,
        hook: KeyRoute<u8>,
    }
    #[derive(Debug, PartialEq)]
    enum To {
        Kabl,
        Host,
    }
    impl Window {
        /// One key message through egui-baseview and the hook.
        fn send(&mut self, owner: Owner, message: KeyMessage, key: &str, code: Code) -> To {
            let mut event = KeyboardEvent::key_down(Key::Character(key.into()), code);
            event.state = if message == KeyMessage::Up {
                KeyState::Up
            } else {
                KeyState::Down
            };
            event.repeat = message == KeyMessage::Repeat;
            let keeps = move |k: Option<egui::Key>, m: egui::Modifiers| owner.keeps(k, m);
            let modifiers = egui::Modifiers::NONE;
            // egui-baseview: a key for the host returns Ignored before egui sees it.
            let status = if key_to_host(&mut self.app, &event, modifiers, &keeps) {
                EventStatus::Ignored
            } else {
                EventStatus::Captured
            };
            let vk = key.to_ascii_uppercase().as_bytes()[0];
            let to_host = self
                .hook
                .to_host(message, vk, || status == EventStatus::Ignored);
            // Delivered to egui exactly when the hook keeps it from the host.
            assert_eq!(to_host, status == EventStatus::Ignored, "{key} {message:?}");
            if to_host {
                To::Host
            } else {
                To::Kabl
            }
        }
        fn focus_lost(&mut self) {
            self.app.clear();
            self.hook.clear();
        }
    }
    use KeyMessage::{Down, Repeat, Up};

    let ctx = egui::Context::default();
    let mut ui = UiState::default();
    let rack = Owner::of(&ui, &ctx);
    ui.browser_open = true;
    ui.browser.selected = Some("factory:init-keyboard".into());
    let browser = Owner::of(&ui, &ctx);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("a text field")));
    let typing = Owner::of(&ui, &ctx);
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("a text field")));
    ui.browser_open = false;
    ui.keys.computer = true;
    let notes = Owner::of(&ui, &ctx);

    let mut w = Window::default();
    // Space: the host's with the browser closed (Live's transport), kabl's preview in it.
    for m in [Down, Repeat, Up] {
        assert_eq!(w.send(rack, m, " ", Code::Space), To::Host, "{m:?}");
    }
    for m in [Down, Repeat, Up] {
        assert_eq!(w.send(browser, m, " ", Code::Space), To::Kabl, "{m:?}");
    }
    // With computer keys off, W is the host's; with them on, kabl plays it once and the host
    // never sees it, auto-repeat included.
    for m in [Down, Repeat, Up] {
        assert_eq!(w.send(rack, m, "w", Code::KeyW), To::Host, "{m:?}");
    }
    for m in [Down, Repeat, Repeat, Up] {
        assert_eq!(w.send(notes, m, "w", Code::KeyW), To::Kabl, "{m:?}");
    }

    // Space held while a text field gains focus: its repeats and key-up stay with the host.
    assert_eq!(w.send(rack, Down, " ", Code::Space), To::Host);
    assert_eq!(w.send(typing, Repeat, " ", Code::Space), To::Host);
    assert_eq!(w.send(typing, Up, " ", Code::Space), To::Host);
    // Space typed into a field that loses focus while it is held: all of it stays with kabl.
    assert_eq!(w.send(typing, Down, " ", Code::Space), To::Kabl);
    assert_eq!(w.send(rack, Repeat, " ", Code::Space), To::Kabl);
    assert_eq!(w.send(rack, Up, " ", Code::Space), To::Kabl);

    // The window loses focus with Space (host) held: a later stray key-up is not the host's.
    assert_eq!(w.send(rack, Down, " ", Code::Space), To::Host);
    w.focus_lost();
    assert_eq!(w.send(rack, Up, " ", Code::Space), To::Kabl);
}
