use kabl_core::{PortRef, Vec2};
use kabl_modules::builtins::seq::bank_param;
use kabl_ui::{banks, PatchEditor};

pub fn prepare(e: &mut PatchEditor, probability: bool) {
    let macros = e.add_module(
        "macro",
        Vec2 {
            x: 24.0,
            y: kabl_ui::rack::row_y(4),
        },
    );
    for (n, name, targets) in [
        (
            1,
            "Glow",
            vec![(9, "cutoff_hz", 0.18), (13, "cutoff_hz", 0.12)],
        ),
        (2, "Motion", vec![(5, "rate_hz", 0.02)]),
        (
            3,
            "Length",
            vec![(10, "decay_ms", 0.06), (14, "release_ms", 0.04)],
        ),
        (
            4,
            if probability { "Echo" } else { "Air" },
            if probability {
                vec![(16, "mix", 0.18), (16, "feedback", 0.12)]
            } else {
                vec![(13, "resonance", 0.14)]
            },
        ),
    ] {
        let key = format!("m{n}");
        e.set_param(macros, &key, 0.3);
        e.set_label(macros, &format!("name.{key}"), Some(name.into()));
        e.set_presentation(&[
            (macros, format!("pin.{key}"), Some(n as f32)),
            (macros, format!("cc.{key}"), Some((19 + n) as f32)),
        ]);
        for (id, param, amount) in targets {
            let cable = e.connect_route(
                PortRef::Module {
                    id: macros,
                    port: key.clone(),
                },
                id,
                param,
            );
            e.set_route_amount(cable, amount, true);
        }
    }
    e.set_presentation(&[
        (1, "pin.transport".into(), Some(0.0)),
        (1, "pin.bpm".into(), Some(7.0)),
        (1, "btn.run".into(), Some(46.0)),
        (1, "btn.restart".into(), Some(47.0)),
    ]);
    for (s, pitches) in [
        (3, [4.0, 16.0, 11.0, 4.0, 14.0, 7.0, 9.0, 4.0]),
        (4, [4.0, 7.0, 11.0, 9.0, 14.0, 4.0, 7.0, 11.0]),
    ] {
        e.set_presentation(&[(s, "pin.banks".into(), Some((s + 2) as f32))]);
        for (b, name) in ["Pulse", "Lift", "Sparse", "Rest"].iter().enumerate() {
            banks::rename(e, s, b, name);
            if b > 0 {
                for (k, pitch) in pitches.iter().enumerate() {
                    e.set_param(s, bank_param(b, k), pitch + if b == 1 { 7.0 } else { 0.0 });
                    e.set_param(
                        s,
                        bank_param(b, 8 + k),
                        if b == 3 || (b == 2 && k % 3 != 0) {
                            0.0
                        } else {
                            1.0
                        },
                    );
                }
                e.set_param(s, bank_param(b, 16), if s == 3 { 7.0 } else { 5.0 });
            }
            if probability {
                for k in 1..8 {
                    e.set_param(s, bank_param(b, 27 + k), if b == 2 { 45.0 } else { 75.0 });
                }
            }
        }
    }
}
