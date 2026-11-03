//! Oscillator family layout tables: the six-operator FM voice.

use super::*;

/// Height of the operator cards, from y 52 to the bottom of the control zone.
const FM6_H: f32 = 226.0;
const FM6_TOP: f32 = 52.0;
/// Left column: algorithm well, four knobs, oversample.
const FM6_LEFT: f32 = 196.0;

/// The six-operator FM voice (PLAN section 4): a left column with the algorithm well, Freq,
/// Fine, Fdbk and Index, and the oversample choice; six operator cards, each with its ratio,
/// Level knob and an envelope curve (drawn and edited by `fm6.rs`). Fine and velocity per
/// operator stay behind `+N`.
pub(super) fn fm6_face(
    info: &'static ModuleInfo,
    controls: &[usize],
    fw: f32,
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) -> Decor {
    let cards = Rect::from_min_max(
        pos2(SIDE + FM6_LEFT + 16.0, FM6_TOP),
        pos2(fw - SIDE, FM6_TOP + FM6_H),
    );
    let blocks = fm6_blocks(cards);
    let pitch = FM6_LEFT / 4.0;
    for &i in controls {
        let name = info.params[i].name;
        let op = name
            .chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .filter(|d| (1..=6).contains(d))
            .map(|d| d as usize - 1);
        let base = op.map_or(name, |_| &name[..name.len() - 1]);
        let col = |k: f32| SIDE + pitch * (k + 0.5);
        let knob_y = FM6_TOP + 76.0 + 17.1 + KNOB_S;
        let geo = match (op, base) {
            (Some(o), "ratio") => Geo::Select {
                rect: Rect::from_min_size(
                    pos2(blocks[o].left() + 6.0, FM6_TOP + 24.0),
                    vec2(blocks[o].width() - 12.0, 20.0),
                ),
            },
            (Some(o), "level") => Geo::Knob {
                c: pos2(blocks[o].center().x, FM6_TOP + 54.0 + 17.1 + KNOB_S),
                r: KNOB_S,
            },
            (None, "algorithm") => Geo::Select {
                rect: Rect::from_min_size(pos2(SIDE + 6.0, FM6_TOP + 4.0), vec2(184.0, 20.0)),
            },
            (None, "base_hz") => Geo::Knob {
                c: pos2(col(0.0), knob_y),
                r: KNOB_S,
            },
            (None, "fine") => Geo::Knob {
                c: pos2(col(1.0), knob_y),
                r: KNOB_S,
            },
            (None, "feedback") => Geo::Knob {
                c: pos2(col(2.0), knob_y),
                r: KNOB_S,
            },
            (None, "index") => Geo::Knob {
                c: pos2(col(3.0), knob_y),
                r: KNOB_S,
            },
            (None, "oversample") => Geo::Select {
                rect: Rect::from_min_size(
                    pos2(SIDE + FM6_LEFT - 80.0, cards.bottom() - 28.0),
                    vec2(80.0, 20.0),
                ),
            },
            // The envelope curve edits these four; they need no place of their own.
            (Some(_), "attack" | "decay" | "sustain" | "release") => continue,
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
    Decor::Operators(cards)
}

/// The six operator cards inside `r`.
pub fn fm6_blocks(r: Rect) -> [Rect; 6] {
    let w = (r.width() - 25.0) / 6.0;
    std::array::from_fn(|i| {
        Rect::from_min_size(
            pos2(r.left() + i as f32 * (w + 5.0), r.top()),
            vec2(w, r.height()),
        )
    })
}

/// Short face label of `param` for the family's kinds (none yet).
pub(super) fn short_name(kind: &str, param: &str) -> Option<&'static str> {
    match (kind, param) {
        ("osc.va", "pw") => Some("PW"),
        ("osc.fm6", "base_hz") => Some("Freq"),
        ("osc.fm6", "feedback") => Some("Fdbk"),
        _ => None,
    }
}

/// VA Oscillator (R1): Fine and Pulse width sit beside Frequency; the other advanced params
/// stay behind `+N`.
pub(super) fn on_face(kind: &str, param: &str) -> bool {
    (kind == "osc.va" && matches!(param, "fine" | "pw"))
        || (kind == "osc.fm6" && matches!(param, "base_hz" | "fine" | "oversample"))
}

/// VA Oscillator: Frequency large, Fine and Pulse width small.
pub(super) fn knob_size(kind: &str, param: &str) -> Option<f32> {
    match (kind, param) {
        ("osc.va", "base_hz") => Some(KNOB_L),
        ("osc.va", "fine" | "pw") => Some(KNOB_S),
        _ => None,
    }
}

/// Default face order of the VA Oscillator's extra knobs: Fine, then Pulse width, right after
/// Waveform (R1 reads Frequency, Fine, Pulse width).
pub(super) fn order(kind: &str, param: &str) -> Option<f32> {
    match (kind, param) {
        ("osc.va", "fine") => Some(1.5),
        ("osc.va", "pw") => Some(1.6),
        _ => None,
    }
}
