//! Timing family layout tables: sequencer, cues, drum grid, clock and MIDI In.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str, fw: f32) -> Option<Decor> {
    Some(match kind {
        // Header row right of the title, ending 8 left of the `+N` chip (G4 mockup).
        "seq" => Decor::Banks(Rect::from_min_size(
            pos2(fw - 54.0 - crate::BANKS_W, 13.0),
            vec2(crate::BANKS_W, 18.0),
        )),
        "cues" => Decor::Cues(Rect::from_min_size(
            pos2(16.0, 64.0),
            vec2(fw - 32.0, 250.0),
        )),
        "drum.grid" => Decor::Grid(Rect::from_min_size(
            pos2(14.0, DISPLAY_TOP + 6.0),
            vec2(fw - 28.0, crate::drum_grid::HEIGHT),
        )),
        "clock" => Decor::Transport(Rect::from_min_size(
            pos2(12.0, 174.0),
            vec2(fw - 24.0, 32.0),
        )),
        _ => return None,
    })
}

/// The MIDI In face, top to bottom: the keys picture with a line naming this module's voice
/// settings, then one full-width row per selector (three options with long names need the
/// width), then a knob row. They must end above the output plate; the picture goes first when
/// room is short, then chosen controls from the last, which move to the advanced area.
pub(super) fn midi_in_face(
    info: &'static ModuleInfo,
    knobs: &[usize],
    sels: &[usize],
    fw: f32,
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) -> Decor {
    const TOP: f32 = 54.0;
    const BOTTOM: f32 = 212.0;
    const SEL_ROW: f32 = 52.0;
    const KNOB_ROW: f32 = 110.0;
    const PICTURE: f32 = 46.0;
    let mut sels = sels.to_vec();
    let mut knobs = knobs.to_vec();
    let height = |s: &[usize], k: &[usize]| {
        s.len() as f32 * SEL_ROW + if k.is_empty() { 0.0 } else { KNOB_ROW }
    };
    while height(&sels, &knobs) > BOTTOM - TOP {
        let last_knob = knobs.last().copied();
        let last_sel = sels.last().copied();
        let drop = match (last_knob, last_sel) {
            (Some(k), Some(s)) if k > s => knobs.pop(),
            (Some(_), None) => knobs.pop(),
            _ => sels.pop(),
        };
        off_face.extend(drop);
    }
    off_face.sort_unstable();
    let picture = height(&sels, &knobs) + PICTURE <= BOTTOM - TOP;
    let decor = if picture {
        // Shorter keys leave two text lines under them: voice settings, then bend and channel.
        Decor::Keys(Rect::from_min_size(pos2(12.0, TOP), vec2(fw - 24.0, 18.0)))
    } else {
        Decor::None
    };
    let mut y = if picture { TOP + PICTURE } else { TOP };
    for &i in &sels {
        let rect = Rect::from_min_size(pos2(14.0, y + 26.0), vec2(fw - 28.0, 28.0));
        ctls.push(Ctl {
            param: &info.params[i],
            primary: true,
            geo: Geo::Select { rect },
        });
        y += SEL_ROW;
    }
    let n = knobs.len();
    for (k, &i) in knobs.iter().enumerate() {
        let x = fw * (k as f32 + 0.5) / n as f32;
        let r = if n == 1 { R_LARGE } else { R_SMALL };
        ctls.push(Ctl {
            param: &info.params[i],
            primary: true,
            geo: Geo::Knob {
                c: pos2(x, y + 60.0),
                r,
            },
        });
    }
    decor
}

/// Sequencer slots the face shows besides the step pitches and gates (G4 mockup).
pub(super) fn on_face(kind: &str, slot: &str) -> bool {
    kind == "seq"
        && (matches!(slot, "length" | "gate_len")
            || slot.strip_prefix('v').is_some_and(|k| k.len() == 1))
}

/// The sequencer's face, measured from the G4 mockup (`g4-surfaces.html`, seq body): a left
/// column with Length and Gate length (r 16, at y 110 and 200, centred 56 from the margin), a
/// divider at x 136, then eight step columns from x 148 to the margin. Per step: pitch knob
/// (r 16, y 124), gate pill (28 x 16, top 176) and velocity bar (6 x 40, top 204); gates and
/// velocities are drawn by the face (`Geo::Select` rects). Anything else the user pins goes
/// to the advanced area.
pub(super) fn seq_face(
    info: &'static ModuleInfo,
    on: &[usize],
    fw: f32,
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) {
    let x0 = SIDE + 136.0;
    let cw = (fw - SIDE - x0) / seq::STEPS as f32;
    for &i in on {
        let param = &info.params[i];
        let slot = seq::slot_name(param.name);
        let step = |pre: char| {
            slot.strip_prefix(pre)
                .and_then(|k| k.parse::<usize>().ok())
                .filter(|k| (1..=seq::STEPS).contains(k))
                .map(|k| x0 + cw * (k as f32 - 0.5))
        };
        let geo = match slot {
            "length" => Geo::Knob {
                c: pos2(SIDE + 56.0, 110.0),
                r: 16.0,
            },
            "gate_len" => Geo::Knob {
                c: pos2(SIDE + 56.0, 200.0),
                r: 16.0,
            },
            _ => {
                if let Some(cx) = step('p') {
                    Geo::Knob {
                        c: pos2(cx, 124.0),
                        r: 16.0,
                    }
                } else if let Some(cx) = step('g') {
                    Geo::Select {
                        rect: Rect::from_min_size(pos2(cx - 14.0, 176.0), vec2(28.0, 16.0)),
                    }
                } else if let Some(cx) = step('v') {
                    Geo::Select {
                        rect: Rect::from_min_size(pos2(cx - 3.0, 204.0), vec2(6.0, 40.0)),
                    }
                } else {
                    off_face.push(i);
                    continue;
                }
            }
        };
        ctls.push(Ctl {
            param,
            primary: true,
            geo,
        });
    }
    off_face.sort_unstable();
}
