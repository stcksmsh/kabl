//! Effects family layout tables.

use super::*;

/// The sparse faces of the mockup (HANDOVER 4.5): a display well on top, a hero knob and two
/// smaller ones under it. The output is a meter well with its Level knob beside it. Positions
/// are the mockup's, in face units; the jacks stay where `layout` puts them.
pub(super) fn sparse_face(
    info: &'static ModuleInfo,
    knobs: &[usize],
    sels: &[usize],
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) -> Decor {
    let knob = |ctls: &mut Vec<Ctl>, name: &str, c: Pos2, r: f32| {
        if let Some(&i) = knobs.iter().find(|&&i| info.params[i].name == name) {
            ctls.push(Ctl {
                param: &info.params[i],
                primary: true,
                geo: Geo::Knob { c, r },
            });
        }
    };
    let decor = match info.kind {
        "out" => {
            knob(ctls, "level_db", pos2(112.0, 104.0), 18.0);
            Decor::Display(Rect::from_min_size(pos2(14.0, 52.0), vec2(62.0, 178.0)))
        }
        "delay" => {
            knob(ctls, "time_ms", pos2(48.0, 160.0), 24.0);
            knob(ctls, "feedback", pos2(124.0, 166.0), 18.0);
            knob(ctls, "mix", pos2(194.0, 166.0), 18.0);
            for &i in sels {
                if info.params[i].name == "sync" {
                    ctls.push(Ctl {
                        param: &info.params[i],
                        primary: true,
                        geo: Geo::Select {
                            rect: Rect::from_min_size(pos2(12.0, 214.0), vec2(216.0, SEL_H)),
                        },
                    });
                }
            }
            Decor::Display(Rect::from_min_size(pos2(12.0, 52.0), vec2(216.0, 54.0)))
        }
        _ => {
            knob(ctls, "decay_s", pos2(48.0, 172.0), 24.0);
            knob(ctls, "damp_hz", pos2(124.0, 178.0), 18.0);
            knob(ctls, "mix", pos2(194.0, 178.0), 18.0);
            Decor::Display(Rect::from_min_size(pos2(12.0, 52.0), vec2(216.0, 54.0)))
        }
    };
    // A primary control this layout has no place for goes to the advanced area.
    for &i in knobs.iter().chain(sels) {
        let placed = ctls.iter().any(|c| std::ptr::eq(c.param, &info.params[i]));
        if !placed {
            off_face.push(i);
        }
    }
    off_face.sort_unstable();
    decor
}

/// Short face label of `param` for the effects (full names stay in the inspector).
pub(super) fn short_name(kind: &str, param: &str) -> Option<&'static str> {
    Some(match (kind, param) {
        ("flanger", "feedback") => "Fdbk",
        ("compressor", "threshold_db") => "Thresh",
        ("bitcrusher", "downsample") => "Rate",
        ("eq", "mid1_hz") => "M1 freq",
        ("eq", "mid1_db") => "M1 gain",
        ("eq", "mid1_q") => "M1 Q",
        ("eq", "mid2_hz") => "M2 freq",
        ("eq", "mid2_db") => "M2 gain",
        ("eq", "mid2_q") => "M2 Q",
        _ => return None,
    })
}
