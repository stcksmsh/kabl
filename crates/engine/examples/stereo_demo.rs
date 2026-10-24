//! Offline render of a patch's stereo test path (`support/stereo.rs`) with a slow auto-pan, as a
//! stereo 32-bit float WAV: a three-note chord, held 5 s, then 3 s of tail.
//!
//!     cargo run --release -p kabl-engine --example stereo_demo -- patches/palette/pad out.wav

use kabl_engine::compile::compile;

#[path = "support/stereo.rs"]
mod stereo;

const RATE: u32 = 48000;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let log = kabl_core::load(std::path::Path::new(&args[0])).expect("patch");
    let patch = stereo::stereo_variant(log.state(), true).expect("a patch with a filter");
    let mut c = compile(&patch, RATE as f32, 8).expect("compiles");
    let blocks = (8 * RATE as usize) / 64;
    let mut out = Vec::with_capacity(blocks * 128);
    for b in 0..blocks {
        if b == 0 {
            for (v, semis) in [-12.0, -5.0, 0.0].into_iter().enumerate() {
                c.note_on(v, semis, 0.8);
            }
        }
        if b == 5 * RATE as usize / 64 {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        c.process_block();
        for i in 0..64 {
            out.extend([c.left()[i], c.right()[i]]);
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
    std::fs::write(&args[1], f).expect("write wav");
}
