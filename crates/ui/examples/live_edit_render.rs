//! Offline render of live editing through the same path `kabl-ui` uses: `PatchEditor` gestures at
//! ~60 fps -> `compile` -> `SwapSender` -> `PatchEngine::drain_swaps` -> `process_block`. Only
//! cpal is missing. For judging clicks, swells and stuck notes by ear without the machine.
//!
//! `cargo run --release -p kabl-ui --example live_edit_render -- docs/modulation-slice/renders`
//!
//! Reference patch, C major 7 chord (4 voices) from 0.2 s. 0.5–3.5 s: Cutoff knob swept up and
//! down. 1.5–3.5 s: the LFO -> Attack route's ring dragged. Keys released at 2.5 s, mid-drag.
//! Written normalized to 0.8 peak (the engine averages 8 voices, so it is quiet raw).

use std::path::Path;

use basedrop::{Collector, Owned};
use kabl_core::ParamTarget;
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{swap_channel, PatchEngine};
use kabl_modules::registry;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;
const FRAME_BLOCKS: usize = 12; // 768 samples = 62.5 UI frames per second

fn render(edit: bool) -> (Vec<f32>, usize) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
    let mut collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, editor.state(), SR, 8).unwrap();
    let (mut tx, mut rx) = swap_channel(4);
    let cutoff = registry::info_for("filter.svf").unwrap().params[0];
    let base_n = cutoff.to_norm(1400.0);

    let blocks = (5.0 * SR) as usize / BLOCK;
    let mut out = Vec::with_capacity(blocks * BLOCK);
    let mut swaps = 0;
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR;
        if edit && b % FRAME_BLOCKS == 0 {
            // One UI frame.
            if (0.5..3.5).contains(&t) {
                let phase = (t - 0.5) / 3.0 * std::f32::consts::TAU;
                let v = cutoff.from_norm(base_n + 0.3 * phase.sin());
                let target = ParamTarget::Module {
                    id: 3,
                    param: "cutoff_hz".into(),
                };
                editor.set_param_gesture(target, v, t < 0.5 + 0.02);
            }
            if (1.5..3.5).contains(&t) {
                let amount = 0.3 + 0.5 * ((t - 1.5) / 2.0);
                editor.set_route_amount(8, amount, t < 1.5 + 0.02);
            }
            if editor.take_dirty() {
                tx.send(Owned::new(&h, compile(editor.state(), SR, 8).unwrap()));
                swaps += 1;
            }
            tx.flush();
            collector.collect();
        }
        engine.drain_swaps(&mut rx);
        if b == (0.2 * SR) as usize / BLOCK {
            for (v, semis) in [0.0, 4.0, 7.0, 11.0].into_iter().enumerate() {
                engine.note_on(v, semis - 12.0, 0.8);
            }
        }
        if b == (2.5 * SR) as usize / BLOCK {
            for v in 0..4 {
                engine.note_off(v);
            }
        }
        engine.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    (out, swaps)
}

fn main() {
    let dir = std::env::args().nth(1).expect("output dir");
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    let (edited, swaps) = render(true);
    let (still, _) = render(false);
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let tail = |s: &[f32]| peak(&s[(4.8 * SR) as usize..]);
    println!("swaps during the render: {swaps}");
    for (name, s) in [("live-edit-drag", &edited), ("live-edit-still", &still)] {
        println!(
            "{name}: peak {:.4}, peak in last 0.2 s {:.5} (a stuck note would stay near the peak)",
            peak(s),
            tail(s)
        );
        let g = 0.8 / peak(s);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(dir.join(format!("{name}.wav")), spec).unwrap();
        for &x in s.iter() {
            w.write_sample(((x * g).clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
    }
}
