//! Portable presentation only. Public IDs remain aliases of the existing leaf targets.
use crate::{Composite, CompositeId, ModuleId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HEIGHT: f32 = 340.0;
pub const MAX_IMAGE_BYTES: usize = 128 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Knob,
    Selector,
    Jack,
    Slider,
    Toggle,
    Icons,
    Display,
    Button,
    CuePads,
    Banks,
    Transport,
    Snapshots,
}
impl Kind {
    /// Bound to a `controls` exposure under the placement's own key.
    pub fn binds_control(self) -> bool {
        matches!(
            self,
            Kind::Knob | Kind::Selector | Kind::Slider | Kind::Toggle | Kind::Icons
        )
    }
    /// Bound to an exposure (a control, or a port for `Jack`) under the placement's own key.
    pub fn binds_exposure(self) -> bool {
        self == Kind::Jack || self.binds_control()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub kind: Kind,
    /// Complete control footprint, including its printed label and value, in rack points.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// What a composite-level element reads or acts on. None for exposure-bound kinds and
    /// for `snapshots`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Display, button and block elements: printed name. Exposure-bound kinds keep using the
    /// exposure label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Icons: one PNG per option, in option order. Empty: the built-in glyph set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<Artwork>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    /// A leaf at any depth; `action` is one `perform::fire` action for a button, None for a
    /// display or a block.
    Module {
        id: ModuleId,
        action: Option<String>,
    },
    /// A `button` placement on a strictly nested composite's face.
    Face { composite: CompositeId, key: u64 },
    /// The snapshot of this composite whose stable `id` this is.
    Snapshot { id: u64 },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artwork {
    /// Local package name, never interpreted as a file path.
    pub name: String,
    pub png: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub width: f32,
    /// Width of the compact face, in 30-point units, 180 or more and below `width`. None: the
    /// whole face always shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_width: Option<f32>,
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub placements: BTreeMap<u64, Placement>,
    pub light: Option<Artwork>,
    pub dark: Option<Artwork>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            width: 300.0,
            compact_width: None,
            placements: BTreeMap::new(),
            light: None,
            dark: None,
        }
    }
}
impl Artwork {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 80
            || self.name == "."
            || self.name == ".."
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err("Artwork name must be a simple local filename; paths are forbidden".into());
        }
        let b = &self.png;
        if b.len() < 33
            || b.len() > MAX_IMAGE_BYTES
            || &b[..8] != b"\x89PNG\r\n\x1a\n"
            || &b[12..16] != b"IHDR"
        {
            return Err("Artwork must be PNG, at most 128 KiB".into());
        }
        let w = u32::from_be_bytes(b[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(b[20..24].try_into().unwrap());
        if w == 0 || h == 0 || w > 2048 || h > 2048 || u64::from(w) * u64::from(h) > 1_048_576 {
            return Err("Artwork dimensions: max 2048 per edge, 1 megapixel total".into());
        }
        Ok(())
    }
}
impl Panel {
    pub fn validate(&self, c: &Composite) -> Result<(), String> {
        if !self.width.is_finite()
            || !(180.0..=840.0).contains(&self.width)
            || self.width % 30.0 != 0.0
        {
            return Err("Panel width: 180–840 points, in 30-point rack units".into());
        }
        if let Some(w) = self.compact_width {
            if !w.is_finite() || w < 180.0 || w >= self.width || w % 30.0 != 0.0 {
                return Err(
                    "Compact width: 180 points or more, below the panel width, in 30-point rack units"
                        .into(),
                );
            }
        }
        for art in [&self.light, &self.dark].into_iter().flatten() {
            art.validate()?;
        }
        let mut rects = Vec::new();
        for (&id, p) in &self.placements {
            let valid = match p.kind {
                Kind::Jack => c.ports.contains_key(&id),
                k if k.binds_control() => c.controls.contains_key(&id),
                // A fresh key from `next_interface`, never an exposure's.
                _ => {
                    id < c.next_interface
                        && !c.controls.contains_key(&id)
                        && !c.ports.contains_key(&id)
                }
            };
            if !valid {
                return Err(format!(
                    "Panel binding {id}: missing exposed ID or wrong control kind"
                ));
            }
            let source_valid = match (p.kind, &p.source) {
                (k, None) => k.binds_exposure() || k == Kind::Snapshots,
                (Kind::Button, Some(Source::Module { action, .. })) => action.is_some(),
                (Kind::Button, Some(Source::Face { .. } | Source::Snapshot { .. })) => true,
                (
                    Kind::Display | Kind::CuePads | Kind::Banks | Kind::Transport,
                    Some(Source::Module { action: None, .. }),
                ) => true,
                _ => false,
            };
            if !source_valid {
                return Err(format!(
                    "Panel binding {id}: source does not fit the element kind"
                ));
            }
            if p.label.as_ref().is_some_and(|l| l.chars().count() > 40) {
                return Err(format!(
                    "Panel binding {id}: label longer than 40 characters"
                ));
            }
            if !p.icons.is_empty() && p.kind != Kind::Icons {
                return Err(format!(
                    "Panel binding {id}: icons only on an icon selector"
                ));
            }
            for art in &p.icons {
                art.validate()?;
            }
            // Provisional sizes for the new kinds; the drawing slices set them from captures.
            let min = match p.kind {
                Kind::Knob => (70.0, 110.0),
                Kind::Selector => (80.0, 55.0),
                Kind::Jack => (54.0, 58.0),
                // Horizontal 120x55 or vertical 40x160.
                Kind::Slider if p.height >= 160.0 => (40.0, 160.0),
                Kind::Slider => (120.0, 55.0),
                Kind::Toggle => (64.0, 55.0),
                // FACES.md asks 18 points per option; core has no option count, so the Icons
                // slice replaces this flat minimum when it draws.
                Kind::Icons => (36.0, 55.0),
                Kind::Display => (120.0, 60.0),
                Kind::Button | Kind::Snapshots => (54.0, 40.0),
                Kind::CuePads => (120.0, 120.0),
                Kind::Banks => (120.0, 18.0),
                Kind::Transport => (120.0, 32.0),
            };
            if [p.x, p.y, p.width, p.height].iter().any(|v| !v.is_finite())
                || p.width < min.0
                || p.height < min.1
                || p.x < 8.0
                || p.y < 64.0
                || p.x + p.width > self.width - 8.0
                || p.y + p.height > HEIGHT - 32.0
            {
                return Err(format!("Panel binding {id}: footprint outside face or too small; reserve header/footer"));
            }
            let r = (p.x, p.y, p.x + p.width, p.y + p.height);
            if rects
                .iter()
                .any(|&(x, y, right, bottom)| r.0 < right && r.2 > x && r.1 < bottom && r.3 > y)
            {
                return Err(format!(
                    "Panel binding {id}: overlapping control footprints"
                ));
            }
            rects.push(r);
        }
        Ok(())
    }
}
