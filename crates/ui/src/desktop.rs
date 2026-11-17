//! What the desktop asks of an app: a reduced-motion request and a light or dark colour scheme.
//!
//! winit and baseview report no theme on Linux, so both are read once at start from the
//! settings portal (`org.freedesktop.portal.Settings`), over D-Bus, with a short timeout. No
//! portal, no session bus, or another OS: no preference. Nothing polls. Tests build [`Hints`]
//! by hand; only [`read`] touches the system.

use serde::{Deserialize, Serialize};

/// What the desktop reported. `Default` is "no preference".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hints {
    /// The portal's `color-scheme`: `Some(true)` dark, `Some(false)` light, `None` no
    /// preference (0, or not reported).
    pub dark: Option<bool>,
    /// Any of the portal, GNOME or KDE asks for less motion.
    pub reduce_motion: bool,
}

/// The raw values behind [`Hints`], as the portal returned them (`None` = not reported).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Raw {
    /// `org.freedesktop.appearance color-scheme`: 0 none, 1 dark, 2 light.
    pub color_scheme: Option<u32>,
    /// `org.freedesktop.appearance reduced-motion`: 1 = reduce (0 none).
    pub portal_reduced_motion: Option<u32>,
    /// GNOME `org.gnome.desktop.interface enable-animations`.
    pub gnome_animations: Option<bool>,
    /// KDE `kdeglobals [KDE] AnimationDurationFactor` (0 = reduce).
    pub kde_duration_factor: Option<f64>,
}

impl Hints {
    pub fn from_raw(r: Raw) -> Hints {
        Hints {
            dark: match r.color_scheme {
                Some(1) => Some(true),
                Some(2) => Some(false),
                _ => None,
            },
            reduce_motion: r.portal_reduced_motion == Some(1)
                || r.gnome_animations == Some(false)
                || r.kde_duration_factor.is_some_and(|f| f <= 0.0),
        }
    }
}

/// Setting "Theme": light or dark from the desktop, or fixed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    /// Dark or light: a fixed choice stands; System follows the desktop and is dark when it
    /// reports nothing.
    pub fn dark(self, hints: Hints) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => hints.dark.unwrap_or(true),
        }
    }
}

/// Setting "Reduce motion".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReduceMotion {
    #[default]
    System,
    On,
    Off,
}

impl ReduceMotion {
    pub fn reduced(self, hints: Hints) -> bool {
        match self {
            ReduceMotion::On => true,
            ReduceMotion::Off => false,
            ReduceMotion::System => hints.reduce_motion,
        }
    }
}

/// Asks the desktop. Blocks for a few ms (a session bus round trip) at most 300 ms.
pub fn read() -> Hints {
    #[cfg(target_os = "linux")]
    {
        portal().map_or_else(Hints::default, Hints::from_raw)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Hints::default()
    }
}

#[cfg(target_os = "linux")]
fn portal() -> Option<Raw> {
    use dbus::arg::{RefArg, Variant};
    use dbus::blocking::Connection;
    use std::collections::HashMap;
    use std::time::Duration;

    type Ns = HashMap<String, HashMap<String, Variant<Box<dyn RefArg>>>>;
    let conn = Connection::new_session().ok()?;
    let proxy = conn.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_millis(300),
    );
    let (all,): (Ns,) = proxy
        .method_call(
            "org.freedesktop.portal.Settings",
            "ReadAll",
            (vec![
                "org.freedesktop.appearance",
                "org.gnome.desktop.interface",
                "org.kde.kdeglobals.KDE",
            ],),
        )
        .ok()?;
    let get = |ns: &str, key: &str| all.get(ns)?.get(key).map(|v| &*v.0);
    Some(Raw {
        color_scheme: get("org.freedesktop.appearance", "color-scheme")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        portal_reduced_motion: get("org.freedesktop.appearance", "reduced-motion")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        gnome_animations: get("org.gnome.desktop.interface", "enable-animations")
            .and_then(|v| v.as_i64())
            .map(|v| v != 0),
        kde_duration_factor: get("org.kde.kdeglobals.KDE", "AnimationDurationFactor")
            .and_then(|v| v.as_f64()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme(c: Option<u32>) -> Hints {
        Hints::from_raw(Raw {
            color_scheme: c,
            ..Raw::default()
        })
    }

    #[test]
    fn the_theme_resolves_by_setting_and_portal() {
        // (portal value, System, Dark, Light)
        for (portal, system) in [
            (Some(0), true),
            (Some(1), true),
            (Some(2), false),
            (None, true),
            (Some(7), true),
        ] {
            let h = scheme(portal);
            assert_eq!(
                ThemeMode::System.dark(h),
                system,
                "System, portal {portal:?}"
            );
            assert!(ThemeMode::Dark.dark(h), "Dark, portal {portal:?}");
            assert!(!ThemeMode::Light.dark(h), "Light, portal {portal:?}");
        }
        assert_eq!(ThemeMode::default(), ThemeMode::System);
    }

    #[test]
    fn any_desktop_source_can_ask_for_reduced_motion() {
        let none = Hints::from_raw(Raw::default());
        assert!(!none.reduce_motion);
        let asks = |r: Raw| Hints::from_raw(r).reduce_motion;
        assert!(asks(Raw {
            portal_reduced_motion: Some(1),
            ..Raw::default()
        }));
        assert!(!asks(Raw {
            portal_reduced_motion: Some(0),
            ..Raw::default()
        }));
        assert!(asks(Raw {
            gnome_animations: Some(false),
            ..Raw::default()
        }));
        assert!(!asks(Raw {
            gnome_animations: Some(true),
            ..Raw::default()
        }));
        assert!(asks(Raw {
            kde_duration_factor: Some(0.0),
            ..Raw::default()
        }));
        assert!(!asks(Raw {
            kde_duration_factor: Some(1.0),
            ..Raw::default()
        }));
    }

    #[test]
    fn the_setting_overrides_the_desktop() {
        let asks = Hints {
            reduce_motion: true,
            dark: None,
        };
        assert!(ReduceMotion::System.reduced(asks));
        assert!(!ReduceMotion::System.reduced(Hints::default()));
        assert!(ReduceMotion::On.reduced(Hints::default()));
        assert!(!ReduceMotion::Off.reduced(asks));
    }
}
