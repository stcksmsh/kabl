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
fn ignored_keys_go_to_the_host_and_key_ups_follow_their_key_downs() {
    use nice_plug_egui::baseview::key_route::{KeyMessage::*, KeyRoute};
    use nice_plug_egui::baseview::EventStatus::{Captured, Ignored};

    const SPACE: u8 = 0x20;
    const A: u8 = 0x41;
    let mut route = KeyRoute::default();

    // An ignored key goes to the host, and so does its key-up.
    assert!(route.to_host(Down, SPACE, Some(Ignored)));
    assert!(route.to_host(Down, SPACE, Some(Ignored)), "auto-repeat");
    assert!(route.to_host(Up, SPACE, None));
    // A used key stays with kabl, and so does its key-up.
    assert!(!route.to_host(Down, A, Some(Captured)));
    assert!(!route.to_host(Up, A, None));
    // Characters, and keys the window made no event of, stay with kabl.
    assert!(!route.to_host(Char, A, None));
    assert!(!route.to_host(Down, A, None));

    // A text field gains focus while Space is held: its key-up still goes where the down went.
    assert!(route.to_host(Down, SPACE, Some(Ignored)));
    assert!(route.to_host(Up, SPACE, Some(Captured)));
    // It loses focus while A is held: A's key-up stays with kabl.
    assert!(!route.to_host(Down, A, Some(Captured)));
    assert!(!route.to_host(Up, A, Some(Ignored)));
    // A key-up whose key-down was never seen stays with kabl.
    assert!(!route.to_host(Up, SPACE, Some(Ignored)));
}
