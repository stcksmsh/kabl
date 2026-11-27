//! Where the Windows keyboard hook sends a key message after the window handler has seen it.
//!
//! The hook takes key messages addressed to baseview windows before the host's message loop
//! can, because some hosts (Ableton Live in particular) would otherwise keep them. Messages the
//! handler did not use go on to the host. The decision is kept apart from the hook so it can be
//! tested on every platform.

use crate::EventStatus;

/// The kind of key message the hook took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMessage {
    /// `WM_KEYDOWN` or `WM_SYSKEYDOWN`.
    Down,
    /// `WM_KEYUP` or `WM_SYSKEYUP`.
    Up,
    /// `WM_CHAR` or `WM_SYSCHAR`.
    Char,
}

/// Remembers which key-downs went to the host, by virtual-key code, so their key-ups follow.
#[derive(Debug, Clone)]
pub struct KeyRoute {
    passed: [bool; 256],
}

impl Default for KeyRoute {
    fn default() -> Self {
        Self { passed: [false; 256] }
    }
}

impl KeyRoute {
    /// Whether the message goes on to the host, given what the handler made of it (`None`: the
    /// handler got no event).
    ///
    /// A key-down goes to the host only when the handler ignored it. A key-up goes where its
    /// key-down went, so neither side is left with a stuck key. Characters stay with the window:
    /// the hook only sees those the window's own key-downs produced.
    pub fn to_host(&mut self, message: KeyMessage, vk: u8, status: Option<EventStatus>) -> bool {
        let slot = &mut self.passed[vk as usize];
        match message {
            KeyMessage::Down => {
                *slot = status == Some(EventStatus::Ignored);
                *slot
            }
            KeyMessage::Up => std::mem::take(slot),
            KeyMessage::Char => false,
        }
    }
}
