use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::c_int,
    ptr,
    sync::{LazyLock, RwLock},
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetParent, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, HHOOK, MSG, PM_REMOVE,
        WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
        WM_USER,
    },
};

use super::*;
use crate::key_route::{KeyMessage, KeyRoute};
use crate::platform::win::window::KEY_EVENT_STATUS;
use crate::wrappers::win32::window::wnd_proc;

// track all windows opened by this instance of baseview
// we use an RwLock here since the vast majority of uses (event interceptions)
// will only need to read from the HashSet
static HOOK_STATE: LazyLock<RwLock<KeyboardHookState>> = LazyLock::new(RwLock::default);

thread_local! {
    // The hook runs on the thread that installed it, so the routing is per thread too.
    static KEY_ROUTE: RefCell<KeyRoute> = RefCell::new(KeyRoute::default());
}

pub(crate) struct KeyboardHookHandle(HWNDWrapper);

#[derive(Default)]
struct KeyboardHookState {
    hook: Option<HHOOK>,
    open_windows: HashSet<HWNDWrapper>,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct HWNDWrapper(HWND);

// SAFETY: it's a pointer behind an RwLock. we'll live
unsafe impl Send for KeyboardHookState {}
unsafe impl Sync for KeyboardHookState {}

// SAFETY: we never access the underlying HWND ourselves, just use it as a HashSet entry
unsafe impl Send for HWNDWrapper {}
unsafe impl Sync for HWNDWrapper {}

impl Drop for KeyboardHookHandle {
    fn drop(&mut self) {
        deinit_keyboard_hook(self.0);
    }
}

// initialize keyboard hook
// some DAWs (particularly Ableton) intercept incoming keyboard messages,
// but we're naughty so we intercept them right back
pub(crate) fn init_keyboard_hook(hwnd: HWND) -> KeyboardHookHandle {
    let state = &mut *HOOK_STATE.write().unwrap();

    // register hwnd to global window set
    state.open_windows.insert(HWNDWrapper(hwnd));

    if state.hook.is_some() {
        // keyboard hook already exists, just return handle
        KeyboardHookHandle(HWNDWrapper(hwnd))
    } else {
        // keyboard hook doesn't exist (no windows open before this), create it
        let new_hook = unsafe {
            SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(keyboard_hook_callback),
                GetModuleHandleW(ptr::null()),
                GetCurrentThreadId(),
            )
        };

        state.hook = Some(new_hook);

        KeyboardHookHandle(HWNDWrapper(hwnd))
    }
}

fn deinit_keyboard_hook(hwnd: HWNDWrapper) {
    let state = &mut *HOOK_STATE.write().unwrap();

    state.open_windows.remove(&hwnd);

    if state.open_windows.is_empty() {
        if let Some(hhook) = state.hook {
            unsafe {
                UnhookWindowsHookEx(hhook);
            }

            state.hook = None;
        }
    }
}

unsafe extern "system" fn keyboard_hook_callback(
    n_code: c_int, wparam: WPARAM, lparam: LPARAM,
) -> isize {
    let msg = lparam as *mut MSG;

    if n_code == HC_ACTION as i32 && wparam == PM_REMOVE as usize && offer_message_to_baseview(msg)
    {
        *msg = MSG {
            hwnd: ptr::null_mut(),
            message: WM_USER,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };

        0
    } else {
        CallNextHookEx(ptr::null_mut(), n_code, wparam, lparam)
    }
}

// check if `msg` is a keyboard message addressed to a window in
// KeyboardHookState::open_windows. If so, the window handles it, and it is intercepted unless the
// handler did not use it: then it goes on to the host, addressed to the window's parent as if
// the host's own window had focus, so the host's loop can act on it and never dispatches it back
// to the baseview window.
unsafe fn offer_message_to_baseview(msg: *mut MSG) -> bool {
    let msg = &mut *msg;

    // if this isn't a keyboard message, ignore it
    let kind = match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN => KeyMessage::Down,
        WM_KEYUP | WM_SYSKEYUP => KeyMessage::Up,
        WM_CHAR | WM_SYSCHAR => KeyMessage::Char,
        _ => return false,
    };

    if !HOOK_STATE.read().unwrap().open_windows.contains(&HWNDWrapper(msg.hwnd)) {
        return false;
    }

    KEY_EVENT_STATUS.set(None);
    let _ = wnd_proc::<BaseviewWindow>(msg.hwnd, msg.message, msg.wParam, msg.lParam);
    let status = KEY_EVENT_STATUS.take();

    if KEY_ROUTE.with_borrow_mut(|route| route.to_host(kind, msg.wParam as u8, status)) {
        // A window without a parent has no host to pass to.
        let parent = GetParent(msg.hwnd);
        if !parent.is_null() {
            msg.hwnd = parent;
            return false;
        }
    }

    true
}
