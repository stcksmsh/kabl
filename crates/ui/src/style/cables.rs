//! Cable palettes: the colours patch cables take per signal kind, and the colours modulation
//! routes cycle through. A skin names its default palette (`Style::cable_palette`); the
//! "Cable colours" setting can override it. Jacks and the living-cable glow read the same
//! colours, so they always match their cables.

use super::{Roles, Style};
use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CablePalette {
    /// The skin's own signal colours and the route colours as they always were.
    #[default]
    Signal,
    /// Quiet greys stepped by signal kind; routes keep their colours.
    Neutral,
    /// Okabe-Ito hues, distinguishable with the common colour-vision deficiencies.
    Contrast,
    /// Patch cables as the skin has them; every modulation route in one teal (plan F3).
    Modulation,
}

impl CablePalette {
    pub const ALL: [CablePalette; 4] = [
        CablePalette::Signal,
        CablePalette::Neutral,
        CablePalette::Contrast,
        CablePalette::Modulation,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CablePalette::Signal => "By signal",
            CablePalette::Neutral => "Neutral",
            CablePalette::Contrast => "High contrast",
            CablePalette::Modulation => "One modulation colour",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            CablePalette::Signal => "signal",
            CablePalette::Neutral => "neutral",
            CablePalette::Contrast => "contrast",
            CablePalette::Modulation => "modulation",
        }
    }
}

fn h(s: &str) -> Color32 {
    Color32::from_hex(s).expect("valid colour")
}

fn set_kinds(r: &mut Roles, [audio, cv, gate, pitch]: [&str; 4]) {
    (r.audio, r.cv, r.gate, r.pitch) = (h(audio), h(cv), h(gate), h(pitch));
}

impl Style {
    /// This style with cable palette `p` applied over the skin's signal and route colours.
    /// `Signal` leaves them as the skin has them.
    pub fn with_cables(mut self, p: CablePalette) -> Style {
        let dark = self.dark;
        let r = &mut self.roles;
        match p {
            CablePalette::Signal => {}
            CablePalette::Neutral => set_kinds(
                r,
                if dark {
                    ["#E8E4DA", "#AEB4BD", "#8A919C", "#C9CED6"]
                } else {
                    ["#2E2C28", "#545962", "#6E727A", "#3F444C"]
                },
            ),
            CablePalette::Contrast => {
                set_kinds(
                    r,
                    if dark {
                        ["#E69F00", "#56B4E9", "#F0E442", "#CC79A7"]
                    } else {
                        ["#D55E00", "#0072B2", "#007A5A", "#A8467E"]
                    },
                );
                let routes = if dark {
                    [
                        "#E69F00", "#56B4E9", "#009E73", "#F0E442", "#3D9BE0", "#D55E00",
                        "#CC79A7", "#F2F2F2",
                    ]
                } else {
                    [
                        "#D55E00", "#0072B2", "#009E73", "#E69F00", "#CC79A7", "#56B4E9",
                        "#333333", "#8A6A00",
                    ]
                };
                self.route_colors = routes.map(h);
            }
            // F3's "one modulation colour" (gate c, branch `claude/mod-colour`): the teal.
            CablePalette::Modulation => {
                self.route_colors = [h(if dark { "#35C2B1" } else { "#12958A" }); 8];
            }
        }
        self.cable_palette = p;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{
        builtin_a, builtin_b, contrast, line_solid_default, look_b_default, outline_default,
    };

    /// The colours each palette sets keep the mark minimum against the rack in every built-in
    /// skin and mode: signal colours 3:1 on dark racks and 2:1 on the pale ones (where cables
    /// carry a casing); route colours 3:1 on dark racks (on pale ones the casing carries them,
    /// as `resting_cables_keep_contrast_on_every_builtin_surface` checks).
    #[test]
    fn every_palette_reads_on_every_rack() {
        let skins: [fn(bool) -> Style; 5] = [
            builtin_a,
            builtin_b,
            look_b_default,
            line_solid_default,
            outline_default,
        ];
        for p in CablePalette::ALL {
            for f in skins {
                for dark in [false, true] {
                    let st = f(dark).with_cables(p);
                    let r = &st.roles;
                    let mut marks = Vec::new();
                    if matches!(p, CablePalette::Neutral | CablePalette::Contrast) {
                        let min = if dark { 3.0 } else { 2.0 };
                        marks.extend([r.audio, r.cv, r.gate, r.pitch].map(|c| (c, min)));
                    }
                    if dark {
                        marks.extend(st.route_colors.map(|c| (c, 3.0)));
                    }
                    for (c, min) in marks {
                        let k = contrast(c, r.rack);
                        assert!(k >= min, "{p:?} {} dark={dark} {c:?}: {k:.2}", st.id);
                    }
                }
            }
        }
    }

    #[test]
    fn signal_palette_is_the_skin_unchanged() {
        for dark in [false, true] {
            assert_eq!(
                look_b_default(dark).with_cables(CablePalette::Signal),
                look_b_default(dark)
            );
        }
    }
}
