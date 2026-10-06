//! Portable presentation only. Public IDs remain aliases of the existing leaf targets.
use crate::Composite;
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
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub placements: BTreeMap<u64, Placement>,
    pub light: Option<Artwork>,
    pub dark: Option<Artwork>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            width: 300.0,
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
        for art in [&self.light, &self.dark].into_iter().flatten() {
            art.validate()?;
        }
        let mut rects = Vec::new();
        for (&id, p) in &self.placements {
            let valid = match p.kind {
                Kind::Jack => c.ports.contains_key(&id),
                _ => c.controls.contains_key(&id),
            };
            if !valid {
                return Err(format!(
                    "Panel binding {id}: missing exposed ID or wrong control kind"
                ));
            }
            let min = match p.kind {
                Kind::Knob => (70.0, 110.0),
                Kind::Selector => (80.0, 55.0),
                Kind::Jack => (54.0, 58.0),
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
