//! Oscillator family layout tables: the six-operator FM voice.

use super::*;

const FM6_H: f32 = 214.0;

/// The six-operator FM voice. A header band holds the jacks, feedback, index and the algorithm
/// choice; under it one framed block per operator: ratio across the top, then fine, level and
/// velocity, then the four envelope knobs. Other chosen controls go to the advanced area.
pub(super) fn fm6_face(
    info: &'static ModuleInfo,
    controls: &[usize],
    fw: f32,
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) -> Decor {
    const TOP: f32 = 100.0;
    let blocks = fm6_blocks(Rect::from_min_size(pos2(10.0, TOP), vec2(fw - 20.0, FM6_H)));
    for &i in controls {
        let name = info.params[i].name;
        let op = name
            .chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .filter(|d| (1..=6).contains(d))
            .map(|d| d as usize - 1);
        let base = op.map_or(name, |_| &name[..name.len() - 1]);
        let b = blocks[op.unwrap_or(0)];
        let (bx, bw) = (b.left(), b.width());
        let row1 = |k: usize| pos2(bx + bw * (k as f32 + 0.5) / 3.0, TOP + 87.0);
        let row2 = |k: usize| pos2(bx + bw * (k as f32 + 0.5) / 4.0, TOP + 171.0);
        let geo = match (op, base) {
            (Some(_), "ratio") => Geo::Select {
                rect: Rect::from_min_size(pos2(bx + 10.0, TOP + 20.0), vec2(bw - 20.0, 20.0)),
            },
            (Some(_), "fine") => Geo::Knob {
                c: row1(0),
                r: R_SMALL,
            },
            (Some(_), "level") => Geo::Knob {
                c: row1(1),
                r: R_SMALL,
            },
            (Some(_), "vel") => Geo::Knob {
                c: row1(2),
                r: R_SMALL,
            },
            (Some(_), "attack") => Geo::Knob {
                c: row2(0),
                r: R_SMALL,
            },
            (Some(_), "decay") => Geo::Knob {
                c: row2(1),
                r: R_SMALL,
            },
            (Some(_), "sustain") => Geo::Knob {
                c: row2(2),
                r: R_SMALL,
            },
            (Some(_), "release") => Geo::Knob {
                c: row2(3),
                r: R_SMALL,
            },
            (None, "feedback") => Geo::Knob {
                c: pos2(430.0, 60.0),
                r: R_SMALL,
            },
            (None, "index") => Geo::Knob {
                c: pos2(490.0, 60.0),
                r: R_SMALL,
            },
            (None, "algorithm") => Geo::Select {
                rect: Rect::from_min_size(pos2(fw - 400.0, 40.0), vec2(220.0, 28.0)),
            },
            _ => {
                off_face.push(i);
                continue;
            }
        };
        ctls.push(Ctl {
            param: &info.params[i],
            primary: true,
            geo,
        });
    }
    off_face.sort_unstable();
    Decor::Operators(Rect::from_min_size(pos2(10.0, TOP), vec2(fw - 20.0, FM6_H)))
}

/// The six operator blocks inside `r`. A wider gap down the middle keeps the face caption
/// clear of the controls.
pub fn fm6_blocks(r: Rect) -> [Rect; 6] {
    let w = (r.width() - 55.0) / 6.0;
    std::array::from_fn(|i| {
        let x = r.left() + i as f32 * (w + 3.0) + if i >= 3 { 40.0 } else { 0.0 };
        Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height()))
    })
}
