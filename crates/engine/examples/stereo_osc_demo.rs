//! Offline render of a played chord phrase on a unison saw (`osc.va` `stereo` port through an
//! envelope and a VCA), one continuous stereo 32-bit float WAV: 12 s with `width` swept
//! 0 -> 100 % -> 0 by a slow LFO, then 8 s of mono / width 100 A/B with 20 ms crossfades.
//!
//!     cargo run --release -p kabl-engine --example stereo_osc_demo -- out.wav

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;

const RATE: u32 = 48000;

/// `Some(width)`: fixed width. `None`: width 50 % moved by an LFO (sine, 12 s period; see `main`) to give 0 -> 100 -> 0 %.
fn patch(width: Option<f32>) -> PatchState {
    let mut s = PatchState::default();
    let module = |kind: &str, params: &[(&str, f32)]| ModuleState {
        kind: kind.into(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    };
    s.modules.insert(1, module("midi.in", &[]));
    s.modules.insert(
        2,
        module(
            "osc.va",
            &[
                ("unison", 8.0),
                ("detune", 30.0),
                ("width", width.unwrap_or(50.0)),
            ],
        ),
    );
    s.modules.insert(3, module("out", &[]));
    s.modules.insert(
        4,
        module("env.adsr", &[("attack_ms", 30.0), ("release_ms", 250.0)]),
    );
    s.modules.insert(5, module("vca", &[]));
    let port = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    let mut cables = vec![
        (port(1, "pitch"), port(2, "pitch")),
        (port(1, "gate"), port(4, "gate")),
        (port(4, "out"), port(5, "cv")),
        (port(2, "stereo"), port(5, "in")),
        (port(5, "out"), port(3, "in")),
    ];
    if width.is_none() {
        s.modules
            .insert(6, module("lfo", &[("rate_hz", 1.0 / 12.0)]));
        cables.push((
            port(6, "out"),
            PortRef::Param {
                id: 2,
                param: "width".into(),
            },
        ));
    }
    for (k, (from, to)) in cables.into_iter().enumerate() {
        let params = if matches!(to, PortRef::Param { .. }) {
            [("amount".to_string(), 0.5)].into()
        } else {
            Default::default()
        };
        s.cables.insert(
            k as u64 + 1,
            CableState {
                from,
                to,
                params,
                steps: Vec::new(),
            },
        );
    }
    s
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let c = |w| compile(&patch(w), RATE as f32, 8).expect("compiles");
    let (mut sweep, mut mono, mut wide) = (c(None), c(Some(0.0)), c(Some(100.0)));
    // Four chords, 1.5 s each, held 1.3 s.
    let chords = [
        [-24.0, -17.0, -12.0],
        [-26.0, -19.0, -14.0],
        [-29.0, -22.0, -17.0],
        [-31.0, -24.0, -19.0],
    ];
    let block = |s: f32| (s * RATE as f32) as usize / 64;
    let blocks = block(20.0);
    let xfade = (0.02 * RATE as f32) as usize;
    // The LFO free-runs from the middle of its sine; 9 s (three quarters of the period) later it
    // is at its trough, which is where the sweep starts.
    for _ in 0..block(9.0) {
        sweep.process_block();
    }
    let mut out = Vec::with_capacity(blocks * 128);
    for b in 0..blocks {
        for e in [&mut sweep, &mut mono, &mut wide] {
            if b % block(1.5) == 0 {
                for (v, semis) in chords[(b / block(1.5)) % 4].into_iter().enumerate() {
                    e.note_on(v, semis, 0.8);
                }
            }
            if b % block(1.5) == block(1.3) {
                for v in 0..3 {
                    e.note_off(v);
                }
            }
            e.process_block();
        }
        for i in 0..64 {
            let t = b * 64 + i; // sample index
            let (l, r) = if b < block(12.0) {
                (sweep.left()[i], sweep.right()[i])
            } else {
                // A/B: 2 s sections starting mono; wide weight ramps over 20 ms at each edge.
                let u = t - 12 * RATE as usize;
                let seg = u / (2 * RATE as usize);
                let into = u % (2 * RATE as usize);
                let w_now = (seg % 2) as f32;
                let w_prev = ((seg + 1) % 2) as f32;
                let w = if seg > 0 && into < xfade {
                    let k = into as f32 / xfade as f32;
                    w_prev + (w_now - w_prev) * k
                } else {
                    w_now
                };
                (
                    mono.left()[i] * (1.0 - w) + wide.left()[i] * w,
                    mono.right()[i] * (1.0 - w) + wide.right()[i] * w,
                )
            };
            out.extend([0.3 * l, 0.3 * r]);
        }
    }
    let data = out.len() as u32 * 4;
    let mut f = Vec::with_capacity(44 + data as usize);
    f.extend_from_slice(b"RIFF");
    f.extend_from_slice(&(36 + data).to_le_bytes());
    f.extend_from_slice(b"WAVEfmt ");
    f.extend_from_slice(&16u32.to_le_bytes());
    f.extend_from_slice(&3u16.to_le_bytes());
    f.extend_from_slice(&2u16.to_le_bytes());
    f.extend_from_slice(&RATE.to_le_bytes());
    f.extend_from_slice(&(RATE * 8).to_le_bytes());
    f.extend_from_slice(&8u16.to_le_bytes());
    f.extend_from_slice(&32u16.to_le_bytes());
    f.extend_from_slice(b"data");
    f.extend_from_slice(&data.to_le_bytes());
    for s in &out {
        f.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, f).expect("write wav");
}
