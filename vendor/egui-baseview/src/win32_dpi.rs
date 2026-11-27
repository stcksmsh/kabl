//! How the editor picks its scale factor on Windows.
//!
//! The window's own DPI (`GetDpiForWindow`) is the one source of truth. baseview reads it once
//! when the window is created and again on `WM_DPICHANGED`, but Windows sends that message only
//! to top-level windows: a plugin editor is a child of the host's window, so when the host
//! window moves to a monitor with another DPI the child gets `WM_DPICHANGED_AFTERPARENT` instead,
//! which baseview ignores. The host's own scale (CLAP `set_scale`, VST3
//! `setContentScaleFactor`) only reaches baseview as a fallback for systems without per-monitor
//! DPI. So the window re-reads its DPI every frame and rescales itself when it changes.

use baseview::dpi::PhysicalSize;

/// Win32 measures DPI against 96 (100 % scaling).
pub fn scale_factor(dpi: u32) -> f64 {
    dpi as f64 / 96.0
}

/// The scale to lay out and render with: the window's DPI when Windows reports one, else the
/// scale baseview reported. `GetDpiForWindow` returns 0 for an invalid window.
pub fn effective_scale(window_dpi: Option<u32>, reported: f64) -> f64 {
    match window_dpi {
        Some(dpi) if dpi > 0 => scale_factor(dpi),
        _ => reported,
    }
}

/// The physical size that keeps the window's logical size when its scale goes from `from` to
/// `to`, or `None` when the scale did not change.
pub fn rescaled(physical: PhysicalSize<u32>, from: f64, to: f64) -> Option<PhysicalSize<u32>> {
    if from == to || from <= 0.0 || to <= 0.0 {
        return None;
    }
    let ratio = to / from;
    Some(PhysicalSize::new(
        (physical.width as f64 * ratio).round() as u32,
        (physical.height as f64 * ratio).round() as u32,
    ))
}

/// Decides when the editor asks the host to fit its frame to the window. The host sizes its frame
/// before the window exists, from the editor's size at scale 1.0, and the window then follows its
/// DPI on its own, so the host has to be told whenever the window's scale moves away from the
/// one it last heard of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFrame {
    asked_scale: f64,
}

impl Default for HostFrame {
    fn default() -> Self {
        Self { asked_scale: 1.0 }
    }
}

impl HostFrame {
    /// Whether to ask the host now, given the scale the window has this frame: when it differs
    /// from the scale the host was last asked for (at first, the 1.0 it sized its frame for). The
    /// scale counts as asked whatever the host answers, so a refusal is never asked again.
    pub fn should_ask(&mut self, scale: f64) -> bool {
        let ask = scale != self.asked_scale;
        self.asked_scale = scale;
        ask
    }
}

/// The DPI of the editor window, or `None` before Windows 10 1607 (no `GetDpiForWindow`) or
/// when the window has no Win32 handle.
#[cfg(windows)]
pub(crate) fn window_dpi(window: &baseview::WindowContext) -> Option<u32> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;
    use std::sync::OnceLock;

    type GetDpiForWindow = unsafe extern "system" fn(isize) -> u32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    // Looked up at run time, as baseview does, so the plugin still loads on older Windows.
    static GET_DPI_FOR_WINDOW: OnceLock<Option<GetDpiForWindow>> = OnceLock::new();
    let get_dpi = (*GET_DPI_FOR_WINDOW.get_or_init(|| {
        let user32: Vec<u16> = "user32.dll\0".encode_utf16().collect();
        // SAFETY: both names are NUL-terminated; user32 is loaded in every process with a window.
        let proc = unsafe {
            let module = GetModuleHandleW(user32.as_ptr());
            if module.is_null() {
                return None;
            }
            GetProcAddress(module, c"GetDpiForWindow".as_ptr().cast())
        };
        // SAFETY: GetDpiForWindow has this signature (winuser.h).
        (!proc.is_null())
            .then(|| unsafe { std::mem::transmute::<*mut c_void, GetDpiForWindow>(proc) })
    }))?;

    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: the handle is the live editor window this context belongs to.
    Some(unsafe { get_dpi(handle.hwnd.get()) })
}

#[cfg(not(windows))]
pub(crate) fn window_dpi(_window: &baseview::WindowContext) -> Option<u32> {
    None
}
