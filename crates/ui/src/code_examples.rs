//! The example code modules as D09/D10 composites (D11 decision: a code module is a leaf, so its
//! panel and labelled exposures come from a one-member composite, exactly like a built-in group).
//!
//! `add` puts a code module on the rack wrapped this way, with plain-language labels instead of
//! raw port ids, and a `Panel` the existing panel renderer draws. Nothing here is specific to
//! wasm: any registered code module with an entry in [`layout_of`] gets the same treatment.

use crate::{composites, PatchEditor};
use kabl_core::composite::Exposure;
use kabl_core::panel::{Artwork, Kind, Panel, Placement};
use kabl_core::{CompositeId, PortRef, Vec2};
use kabl_modules::code::host;
use std::collections::BTreeSet;
use std::io::Cursor;

/// How one exposed port or parameter reads on the face.
struct Item {
    id: &'static str,
    label: &'static str,
    kind: Kind,
    /// Footprint x and y in rack points; the size comes from the kind.
    at: (f32, f32),
}

struct Layout {
    name: &'static str,
    width: f32,
    items: &'static [Item],
}

const KNOB: (f32, f32) = (100.0, 110.0);
const SELECTOR: (f32, f32) = (100.0, 60.0);
const JACK: (f32, f32) = (58.0, 58.0);

const LOGIC: Layout = Layout {
    name: "Logic",
    width: 360.0,
    items: &[
        Item {
            id: "mode",
            label: "Function",
            kind: Kind::Selector,
            at: (12.0, 70.0),
        },
        Item {
            id: "threshold",
            label: "Threshold",
            kind: Kind::Knob,
            at: (122.0, 70.0),
        },
        Item {
            id: "offset",
            label: "Offset",
            kind: Kind::Knob,
            at: (232.0, 70.0),
        },
        Item {
            id: "a",
            label: "Input A",
            kind: Kind::Jack,
            at: (12.0, 190.0),
        },
        Item {
            id: "b",
            label: "Input B",
            kind: Kind::Jack,
            at: (82.0, 190.0),
        },
        Item {
            id: "gate",
            label: "Hold",
            kind: Kind::Jack,
            at: (152.0, 190.0),
        },
        Item {
            id: "out",
            label: "Result",
            kind: Kind::Jack,
            at: (222.0, 190.0),
        },
        Item {
            id: "trig",
            label: "Trigger",
            kind: Kind::Jack,
            at: (292.0, 190.0),
        },
    ],
};

const BITFILTER: Layout = Layout {
    name: "Bit Filter",
    width: 330.0,
    items: &[
        Item {
            id: "cutoff",
            label: "Cutoff",
            kind: Kind::Knob,
            at: (12.0, 70.0),
        },
        Item {
            id: "resonance",
            label: "Resonance",
            kind: Kind::Knob,
            at: (112.0, 70.0),
        },
        Item {
            id: "drive",
            label: "Drive",
            kind: Kind::Knob,
            at: (212.0, 70.0),
        },
        Item {
            id: "in",
            label: "Audio in",
            kind: Kind::Jack,
            at: (12.0, 190.0),
        },
        Item {
            id: "cutoff_cv",
            label: "Cutoff mod",
            kind: Kind::Jack,
            at: (100.0, 190.0),
        },
        Item {
            id: "out",
            label: "Audio out",
            kind: Kind::Jack,
            at: (188.0, 190.0),
        },
    ],
};

fn layout_of(kind: &str) -> Option<&'static Layout> {
    match kind {
        "demo.logic" => Some(&LOGIC),
        "demo.bitfilter" => Some(&BITFILTER),
        _ => None,
    }
}

/// Whether `kind` is a code module with a ready-made panel.
pub fn has_panel(kind: &str) -> bool {
    layout_of(kind).is_some()
}

fn art(name: &str, rgb: [u8; 3]) -> Artwork {
    let edge = rgb.map(|c| c.saturating_add(18));
    let image = image::RgbaImage::from_fn(60, 68, |x, y| {
        let border = x < 2 || y < 2 || x >= 58 || y >= 66;
        let c = if border { edge } else { rgb };
        image::Rgba([c[0], c[1], c[2], 255])
    });
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encoding a small PNG in memory cannot fail");
    Artwork {
        name: name.into(),
        png: bytes.into_inner(),
    }
}

/// The composite for module `leaf` of registered code module `kind`: labelled exposures and a
/// validated panel. `None` when `kind` has no layout.
pub fn composite_for(
    base: kabl_core::Composite,
    kind: &str,
    leaf: u64,
) -> Option<kabl_core::Composite> {
    let layout = layout_of(kind)?;
    let info = kabl_modules::registry::info_for(kind)?;
    let (version, _) = host::blob_for(kind)?;
    let mut c = base;
    c.name = layout.name.into();
    c.help = info.explain.into();
    c.definition = kabl_core::composite::Definition {
        id: kind.into(),
        version,
    };
    c.ports.clear();
    c.controls.clear();
    c.next_interface = 1;
    let mut panel = Panel {
        width: layout.width,
        ..Panel::default()
    };
    for item in layout.items {
        let (target, control) = if info.params.iter().any(|p| p.name == item.id) {
            (
                PortRef::Param {
                    id: leaf,
                    param: item.id.into(),
                },
                true,
            )
        } else if info.ports.iter().any(|p| p.name == item.id) {
            (
                PortRef::Module {
                    id: leaf,
                    port: item.id.into(),
                },
                false,
            )
        } else {
            continue;
        };
        let key = c.next_interface;
        c.next_interface += 1;
        let e = Exposure {
            label: item.label.into(),
            target,
        };
        if control {
            c.controls.insert(key, e);
        } else {
            c.ports.insert(key, e);
        }
        let (w, h) = match item.kind {
            Kind::Knob => KNOB,
            Kind::Selector => SELECTOR,
            Kind::Jack => JACK,
        };
        panel.placements.insert(
            key,
            Placement {
                kind: item.kind,
                x: item.at.0,
                y: item.at.1,
                width: w,
                height: h,
            },
        );
    }
    panel.light = Some(art("panel-light.png", [214, 216, 210]));
    panel.dark = Some(art("panel-dark.png", [46, 49, 52]));
    panel.validate(&c).ok()?;
    c.panel = Some(Box::new(panel));
    Some(c)
}

/// Adds code module `kind` at `pos`, wrapped in its labelled composite with its panel.
/// One undo step per edit, like any other composite.
pub fn add(editor: &mut PatchEditor, kind: &str, pos: Vec2) -> Result<(u64, CompositeId), String> {
    host::register_examples().map_err(|e| e.to_string())?;
    if !has_panel(kind) {
        return Err(format!("`{kind}` has no panel layout"));
    }
    let leaf = editor.add_module(kind, pos);
    let id = composites::encapsulate(editor, BTreeSet::from([leaf]), BTreeSet::new(), kind)?;
    let base = editor.state().composites[&id].clone();
    let c = composite_for(base, kind, leaf).ok_or("The panel doesn't fit this module")?;
    composites::set(editor, id, c)?;
    Ok((leaf, id))
}
