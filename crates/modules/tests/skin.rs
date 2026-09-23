//! `ModuleSkin` correctness (owner ask, not a brief feature — see docs/decisions.md "Module
//! skins: custom panel art"): every skinned module's `ControlSkin`s must actually name real ports
//! and params of that same module, and must cover every one of them — a skin that misses a port
//! or param makes it unreachable from a skin-aware renderer, and a skin that names a stale one
//! (e.g. after a rename) would silently draw nothing for it. Also checks the embedded background
//! image bytes actually decode, catching a corrupted asset at test time instead of only at
//! runtime inside `kabl-ui`.

use kabl_modules::registry;
use kabl_modules::skin::ControlKind;

#[test]
fn every_skin_controls_id_matches_a_real_port_or_param_and_covers_all_of_them() {
    for &kind in registry::KNOWN_KINDS {
        let info = registry::info_for(kind).expect("known kind must resolve");
        let Some(skin) = info.skin else { continue };

        for control in skin.controls {
            match control.kind {
                ControlKind::Jack => assert!(
                    info.ports.iter().any(|p| p.name == control.id),
                    "{kind}'s skin names a Jack control {:?} that isn't one of its real ports",
                    control.id
                ),
                ControlKind::Knob => assert!(
                    info.params.iter().any(|p| p.name == control.id),
                    "{kind}'s skin names a Knob control {:?} that isn't one of its real params",
                    control.id
                ),
                ControlKind::Switch | ControlKind::Readout => {
                    // Declared, not wired to anything real yet -- nothing to check against.
                }
            }
        }

        for port in info.ports {
            assert!(
                skin.controls
                    .iter()
                    .any(|c| c.kind == ControlKind::Jack && c.id == port.name),
                "{kind}'s skin doesn't place its {:?} port -- it would be unreachable from a \
                 skin-aware renderer",
                port.name
            );
        }
        for param in info.params {
            assert!(
                skin.controls
                    .iter()
                    .any(|c| c.kind == ControlKind::Knob && c.id == param.name),
                "{kind}'s skin doesn't place its {:?} param -- it would be unreachable from a \
                 skin-aware renderer",
                param.name
            );
        }

        for control in skin.controls {
            let (x, y) = control.pos;
            assert!(
                (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y),
                "{kind}'s skin places {:?} at {:?}, outside the normalized 0..1 panel range",
                control.id,
                control.pos
            );
        }
    }
}

#[test]
fn every_skin_background_image_decodes_as_a_valid_png() {
    for &kind in registry::KNOWN_KINDS {
        let info = registry::info_for(kind).expect("known kind must resolve");
        let Some(skin) = info.skin else { continue };
        for bytes in [skin.background_image, skin.background_dark]
            .into_iter()
            .flatten()
        {
            let decoded = image::load_from_memory(bytes)
                .unwrap_or_else(|e| panic!("{kind}'s skin image failed to decode: {e}"));
            assert!(
                decoded.width() > 0 && decoded.height() > 0,
                "{kind}'s skin image decoded to zero size"
            );
        }
    }
}
