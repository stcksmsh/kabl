//! Offline render of a unison saw on `osc.va`'s `stereo` port at width 0, 50 and 100 %
//! (3 s each, a held chord), as a stereo 32-bit float WAV.
//!
//!     cargo run --release -p kabl-engine --example stereo_osc_demo -- out.wav

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;

const RATE: u32 = 48000;

fn patch(width: f32) -> PatchState {
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
            &[("unison", 8.0), ("detune", 30.0), ("width", width)],
        ),
    );
    s.modules.insert(3, module("out", &[]));
    let port = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    for (k, (from, to)) in [
        (port(1, "pitch"), port(2, "pitch")),
        (port(2, "stereo"), port(3, "in")),
    ]
    .into_iter()
    .enumerate()
    {
        s.cables.insert(
            k as u64 + 1,
            CableState {
                from,
                to,
                params: Default::default(),
                steps: Vec::new(),
            },
        );
    }
    s
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let mut out = Vec::new();
    for width in [0.0, 50.0, 100.0] {
        let mut c = compile(&patch(width), RATE as f32, 8).expect("compiles");
        for (v, semis) in [-24.0, -17.0, -12.0].into_iter().enumerate() {
            c.note_on(v, semis, 0.8);
        }
        for _ in 0..(3 * RATE as usize / 64) {
            c.process_block();
            for i in 0..64 {
                // Quiet: the saw stack is hot.
                out.extend([0.3 * c.left()[i], 0.3 * c.right()[i]]);
            }
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
