//! Whether a key message goes to the window or on to the host, decided once per press.
//!
//! The Windows keyboard hook takes key messages addressed to baseview windows before the host's
//! message loop can, because some hosts (Ableton Live in particular) would otherwise keep them.
//! Messages the handler did not use go on to the host. The decision is kept apart from the hook
//! so it can be tested on every platform, and so a window handler can route its own events the
//! same way.

use std::collections::HashMap;
use std::hash::Hash;

/// The kind of key message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMessage {
    /// The first key-down of a press (`WM_KEYDOWN` or `WM_SYSKEYDOWN`).
    Down,
    /// An auto-repeated key-down.
    Repeat,
    /// `WM_KEYUP` or `WM_SYSKEYUP`.
    Up,
    /// `WM_CHAR` or `WM_SYSCHAR`.
    Char,
}

/// Remembers, per key, where each press went, so its repeats and its key-up follow it.
#[derive(Debug, Clone)]
pub struct KeyRoute<K> {
    to_host: HashMap<K, bool>,
}

impl<K> Default for KeyRoute<K> {
    fn default() -> Self {
        Self { to_host: HashMap::new() }
    }
}

impl<K: Eq + Hash> KeyRoute<K> {
    /// Whether the message goes on to the host. `decide` answers that for a new press and is
    /// asked only then: on a key-down, or on a repeat of a press this route never saw.
    ///
    /// Repeats and the key-up go where the press went, whatever the window's state is by then,
    /// so neither side is left with a stuck or doubled key. A key-up of a press this route
    /// never saw (or forgot in `clear`) stays with the window. Characters stay with the window:
    /// the hook only sees those the window's own key-downs produced.
    pub fn to_host(&mut self, message: KeyMessage, key: K, decide: impl FnOnce() -> bool) -> bool {
        match message {
            KeyMessage::Down => {
                let to_host = decide();
                self.to_host.insert(key, to_host);
                to_host
            }
            KeyMessage::Repeat => *self.to_host.entry(key).or_insert_with(decide),
            KeyMessage::Up => self.to_host.remove(&key).unwrap_or(false),
            KeyMessage::Char => false,
        }
    }

    /// Forgets every press, for when the window loses focus: the key-ups of keys still held
    /// then go to whatever window has focus.
    pub fn clear(&mut self) {
        self.to_host.clear();
    }
}
