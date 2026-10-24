//! Offline renders of the `osc.va` supersaw for listening: the same chord at 1, 4, 8 and 16
//! voices, then 16 voices with `curve` and `width` swept. Writes stereo 16-bit WAVs.
//!
//!     cargo run --release -p kabl-modules --example unison_clips -- docs/unison/clips

use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
/// A minor 9, in semitones from C4 (A2 C3 E3 G3 B3).
const CHORD: [f32; 5] = [-15.0, -12.0, -8.0, -5.0, -1.0];

struct Note {
    osc: Box<dyn Module>,
}

fn note() -> Note {
    let mut osc = registry::create("osc.va").unwrap();
    osc.prepare(
        SR,
        BLOCK,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    Note { osc }
}

/// One chord of `seconds`; `params(t)` gives (unison, detune, curve, width, blend) at time `t`.
fn chord(seconds: f32, params: impl Fn(f32) -> [f32; 5]) -> (Vec<f32>, Vec<f32>) {
    let mut notes: Vec<Note> = CHORD.iter().map(|_| note()).collect();
    let n = (seconds * SR) as usize / BLOCK * BLOCK;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let info = notes[0].osc.info();
    let idx = |name: &str| info.params.iter().position(|p| p.name == name).unwrap();
    let (i_u, i_d, i_c, i_w, i_b) = (
        idx("unison"),
        idx("detune"),
        idx("curve"),
        idx("width"),
        idx("blend"),
    );
    for b in 0..n / BLOCK {
        let t = (b * BLOCK) as f32 / SR;
        let [u, d, c, w, bl] = params(t);
        for (k, note) in notes.iter_mut().enumerate() {
            let mut ps: Vec<f32> = info.params.iter().map(|p| p.default).collect();
            ps[idx("base_hz")] = 261.63;
            ps[i_u] = u;
            ps[i_d] = d;
            ps[i_c] = c;
            ps[i_w] = w;
            ps[i_b] = bl;
            let ps: Vec<Signal> = ps.iter().map(|&v| Signal::Scalar(v)).collect();
            let pitch = [CHORD[k]; BLOCK];
            let none = [0.0f32; BLOCK];
            let ins = [Signal::Buffer(&pitch), Signal::Buffer(&none)];
            let (mut ol, mut or) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
            {
                let mut outs: [&mut [f32]; 2] = [&mut ol, &mut or];
                note.osc
                    .process(&mut ProcessIo::new(&ins, &mut outs, &ps, BLOCK));
            }
            for i in 0..BLOCK {
                l[b * BLOCK + i] += ol[i] * 0.07;
                r[b * BLOCK + i] += or[i] * 0.07;
            }
        }
    }
    // Attack and release so the clip starts and ends clean.
    for (i, (a, c)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
        let g = (i as f32 / (0.05 * SR)).min(1.0) * ((n - i) as f32 / (0.15 * SR)).min(1.0);
        *a *= g;
        *c *= g;
    }
    (l, r)
}

fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32]) {
    let data = l.len() as u32 * 4;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    for x in [16u32, 0x0002_0001, SR as u32, SR as u32 * 4, 0x0010_0004] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    for (a, b) in l.iter().zip(r) {
        for s in [a, b] {
            v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    std::fs::write(path, v).unwrap();
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&dir).unwrap();
    // Four chords one after another, 2.5 s each, the same detune (40 ct, even steps, mono).
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for count in [1.0, 4.0, 8.0, 16.0] {
        let (a, b) = chord(2.5, |_| [count, 40.0, 0.0, 0.0, 50.0]);
        l.extend(a);
        r.extend(b);
    }
    write_wav(&dir.join("unison-1-4-8-16.wav"), &l, &r);
    // 16 voices, 12 s: detune and curve rise together, then width opens, then blend sweeps.
    let (l, r) = chord(12.0, |t| {
        let detune = 40.0;
        let curve = (t / 4.0).clamp(0.0, 1.0) * 100.0;
        let width = ((t - 4.0) / 4.0).clamp(0.0, 1.0) * 100.0;
        let blend = 50.0 + 50.0 * ((t - 8.0) / 2.0 * std::f32::consts::PI).sin().max(0.0);
        [
            16.0,
            detune,
            curve,
            width,
            if t < 8.0 { 50.0 } else { blend },
        ]
    });
    write_wav(&dir.join("supersaw-curve-width-blend.wav"), &l, &r);
}
