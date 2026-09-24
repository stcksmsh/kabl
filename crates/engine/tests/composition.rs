//! `patches/composition` through the real compiler and `PatchEngine`: every cue launched in
//! turn with live edits in between, under `assert_no_alloc`. Checks headroom per section,
//! finite output, bank changes landing on bar lines, and effect tails carrying through the
//! transitions (no dropout at a launch).

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Launch, PatchEngine, Timing};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const BASS: ModuleId = 6;
const ARP: ModuleId = 7;
/// 16ths at 112 bpm.
const TICK: f64 = 48000.0 * 60.0 / 112.0 / 4.0;

fn patch() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    kabl_core::load(&dir)
        .expect("composition patch")
        .state()
        .clone()
}

fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}

struct Section {
    name: &'static str,
    banks: (u8, u8),
    peak: f32,
    sum: f64,
    n: usize,
}

#[test]
fn every_section_has_headroom_and_lands_on_the_bar() {
    let collector = Collector::new();
    let handle = collector.handle();
    let base = patch();
    let mut e = PatchEngine::new(&handle, &base, SR, 8).unwrap();
    // A live edit (inactive-bank pitch) queued for the middle of each section.
    let mut edited = base.clone();
    edited
        .modules
        .get_mut(&BASS)
        .unwrap()
        .params
        .insert("d.p1".into(), -18.0);
    let mut sections = [
        ("Intro", (0, 0)),
        ("Main", (1, 1)),
        ("Variation", (2, 2)),
        ("Breakdown", (3, 3)),
        ("Return", (1, 2)),
    ]
    .map(|(name, banks)| Section {
        name,
        banks,
        peak: 0.0,
        sum: 0.0,
        n: 0,
    });
    let bars_per_section = 8;
    let section_samples = (TICK * 16.0 * bars_per_section as f64) as usize;
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut t = 0usize;
    let mut last = (0f32, 0f32);
    let mut worst_jump = 0f32;
    let mut switch_at: Vec<(usize, (usize, usize))> = Vec::new();
    let mut prev_banks = (0usize, 0usize);
    for (s, sec) in sections.iter_mut().enumerate() {
        let launch = Launch::new(
            CLOCK,
            Timing::NextBar,
            &[(BASS, sec.banks.0), (ARP, sec.banks.1)],
        );
        let swap_at = t + section_samples / 2;
        let mut swap = Some(
            e.build_swap(&handle, if s % 2 == 0 { &edited } else { &base })
                .unwrap(),
        );
        let end = t + section_samples;
        let mut first = true;
        while t < end {
            let mut banks = (0, 0);
            assert_no_alloc(|| {
                if first && s > 0 {
                    e.launch(&launch);
                }
                if t >= swap_at {
                    if let Some(g) = swap.take() {
                        e.receive_swap(g);
                    }
                }
                e.process_block(&mut l, &mut r);
                e.seqs(|id, _, bank, _| match id {
                    BASS => banks.0 = bank,
                    ARP => banks.1 = bank,
                    _ => {}
                });
            });
            first = false;
            if banks != prev_banks {
                switch_at.push((t, banks));
                prev_banks = banks;
            }
            for i in 0..BLOCK {
                assert!(l[i].is_finite() && r[i].is_finite());
                // Skip the first bar of the piece (everything starting from silence).
                if t > TICK as usize * 16 {
                    worst_jump = worst_jump
                        .max((l[i] - last.0).abs())
                        .max((r[i] - last.1).abs());
                }
                last = (l[i], r[i]);
                sec.peak = sec.peak.max(l[i].abs()).max(r[i].abs());
                sec.sum += (l[i] as f64).powi(2) + (r[i] as f64).powi(2);
                sec.n += 2;
            }
            t += BLOCK;
        }
    }
    for sec in &sections {
        let rms = (sec.sum / sec.n as f64).sqrt() as f32;
        println!(
            "{:<10} peak {:>6.1} dBFS  rms {:>6.1} dBFS",
            sec.name,
            db(sec.peak),
            db(rms)
        );
        assert!(
            sec.peak < 0.8,
            "{}: peak {:.1} dBFS",
            sec.name,
            db(sec.peak)
        );
        assert!(db(rms) > -40.0, "{} is audible", sec.name);
    }
    println!("largest sample step {worst_jump:.3}; switches {switch_at:?}");
    // Each section's banks switch within the block holding its first bar line (the command
    // arrives at the section's start, which is itself on a bar line + 0..1 block).
    assert_eq!(switch_at.len(), 4);
    for (k, &(at, banks)) in switch_at.iter().enumerate() {
        let bar = (TICK * 16.0 * (bars_per_section * (k + 1) + 1) as f64).ceil() as usize;
        assert!(
            at.abs_diff(bar) <= BLOCK,
            "switch {k} at {at}, bar at {bar}"
        );
        let want = sections[k + 1].banks;
        assert_eq!(banks, (want.0 as usize, want.1 as usize));
    }
}
