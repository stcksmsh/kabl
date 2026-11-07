//! Flush-to-zero must not change how self-oscillating and feedback modules behave: a resonant
//! filter at maximum resonance, feedback delays and reverbs, driven by one impulse and then by
//! silence, render the same (within -140 dBFS) with the guard on and off, and with no input at
//! all they stay silent either way, so nothing relies on denormal noise to start.

use kabl_engine::ftz::FtzGuard;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, PortDirection, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const SECONDS: usize = 12;

/// Left output of `kind` with feedback-like params at maximum; `impulse` = 0.1 at sample 0.
fn render(kind: &str, ftz: bool, impulse: bool) -> Vec<f32> {
    let _guard = ftz.then(FtzGuard::new);
    let mut m = registry::create(kind).unwrap();
    m.prepare(
        SR,
        BLOCK,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let info = m.info();
    let params: Vec<f32> = info
        .params
        .iter()
        .map(|p| {
            if ["feedback", "resonance", "decay", "regen", "size"]
                .iter()
                .any(|k| p.name.contains(k))
            {
                p.max
            } else {
                p.default
            }
        })
        .collect();
    let n_in = info.slots(PortDirection::Input);
    let n_out = info.slots(PortDirection::Output);
    let ps: Vec<Signal> = params.iter().map(|&v| Signal::Scalar(v)).collect();
    let mut out = Vec::new();
    for b in 0..SECONDS * SR as usize / BLOCK {
        let mut first = [0f32; BLOCK];
        if impulse && b == 0 {
            first[0] = 0.1;
        }
        let silence = [0f32; BLOCK];
        let ins: Vec<Signal> = (0..n_in)
            .map(|k| Signal::Buffer(if k == 0 { &first } else { &silence }))
            .collect();
        let mut outs = vec![[0f32; BLOCK]; n_out];
        {
            let mut refs: Vec<&mut [f32]> = outs.iter_mut().map(|o| &mut o[..]).collect();
            let mut io = ProcessIo::new(&ins, &mut refs, &ps, BLOCK);
            m.process(&mut io);
        }
        out.extend_from_slice(&outs[0]);
    }
    out
}

#[test]
fn self_oscillating_modules_sound_the_same_with_ftz() {
    let mut worst = 0f32;
    for kind in [
        "filter.ladder",
        "filter.svf",
        "filter.comb",
        "delay",
        "reverb",
        "flanger",
        "phaser",
        "chorus",
        "vocoder",
    ] {
        let off = render(kind, false, true);
        let on = render(kind, true, true);
        let diff = off
            .iter()
            .zip(&on)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        let peak = off.iter().fold(0f32, |m, x| m.max(x.abs()));
        println!("{kind}: peak {peak:.4}, max |off - on| = {diff:e}");
        assert!(diff <= 1e-7, "{kind}: FTZ changes the sound by {diff:e}");
        worst = worst.max(diff);

        // No input: silent with FTZ off, so it cannot depend on denormal noise to start.
        let quiet_off = render(kind, false, false);
        let quiet_on = render(kind, true, false);
        let q = |v: &[f32]| v.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert_eq!(q(&quiet_off), q(&quiet_on), "{kind} with no input");
        assert!(q(&quiet_on) <= 1e-7, "{kind} makes sound from nothing");
    }
    println!("worst {worst:e}");
}
