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
