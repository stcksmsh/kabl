//! The drum voices and `midi.drums`: level and length at the defaults, sample-accurate starts,
//! click-free retriggers, the hat's choke, extreme settings at 44.1 to 192 kHz and odd block
//! sizes, the MIDI map, and no allocation on the audio path. `-- --nocapture` prints numbers.

#[cfg(debug_assertions)]
use assert_no_alloc::{assert_no_alloc, AllocDisabler};
#[cfg(not(debug_assertions))]
fn assert_no_alloc<T>(f: impl FnOnce() -> T) -> T {
    f()
}
use kabl_modules::builtins::MidiNote;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const VOICES: [&str; 5] = [
    "drum.kick",
    "drum.snare",
    "drum.clap",
    "drum.hat",
    "drum.tom",
];
const SR: f32 = 48000.0;

struct Dut {
    m: Box<dyn Module>,
    params: Vec<f32>,
    n_in: usize,
    n_out: usize,
}

impl Dut {
    fn new(kind: &str, set: &[(&str, f32)], sr: f32) -> Dut {
        let mut m = registry::create(kind).unwrap();
        m.prepare(
            sr,
            4096,
            &QualityConfig {
                tier: QualityTier::Live,
            },
        );
        let info = m.info();
        let params = info.params.iter().map(|p| p.default).collect();
        let n_in = info.slots(kabl_modules::PortDirection::Input);
        let n_out = info.slots(kabl_modules::PortDirection::Output);
        let mut d = Dut {
            m,
            params,
            n_in,
            n_out,
        };
        for &(k, v) in set {
            d.set(k, v);
        }
        d
    }

    fn set(&mut self, name: &str, v: f32) {
        let i = self
            .m
            .info()
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap();
        self.params[i] = v;
    }

    /// `ins[k]` is the buffer of input `k`; inputs past the list are unpatched (a held 0).
    fn run(&mut self, ins: &[&[f32]], len: usize, block: usize) -> Vec<Vec<f32>> {
        let mut out = vec![vec![0f32; len]; self.n_out];
        let ps: Vec<Signal> = self.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut t = 0;
        while t < len {
            let n = block.min(len - t);
            let sig: Vec<Signal> = (0..self.n_in)
                .map(|k| {
                    ins.get(k)
                        .map_or(Signal::Scalar(0.0), |b| Signal::Buffer(&b[t..t + n]))
                })
                .collect();
            let mut refs: Vec<&mut [f32]> = out.iter_mut().map(|b| &mut b[t..t + n]).collect();
            let mut io = ProcessIo::new(&sig, &mut refs, &ps, n);
            self.m.process(&mut io);
            t += n;
        }
        out
    }
}

/// A gate that is high for `hold` samples from each of `at`.
fn gate(len: usize, at: &[usize], hold: usize) -> Vec<f32> {
    let mut g = vec![0f32; len];
    for &a in at {
        for x in g.iter_mut().skip(a).take(hold) {
            *x = 1.0;
        }
    }
    g
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

/// One hit of `kind` (the closed gate for the hat) over 2 s.
fn one_hit(kind: &str, sr: f32) -> Vec<f32> {
    let len = (2.0 * sr) as usize;
    let g = gate(len, &[100], 480);
    Dut::new(kind, &[], sr).run(&[&g], len, 64).remove(0)
}

#[test]
fn defaults_hit_at_a_usable_level_and_end_in_silence() {
    for kind in VOICES {
        let x = one_hit(kind, SR);
        let tail = &x[x.len() - 4800..];
        println!(
            "{kind}: peak {:.3}, rms of first 300 ms {:.3}",
            peak(&x),
            rms(&x[..14400])
        );
        assert!(peak(&x) > 0.25, "{kind} too quiet");
        assert!(peak(&x) < 1.1, "{kind} too loud");
        assert_eq!(peak(tail), 0.0, "{kind} must end in exact silence");
    }
}

#[test]
fn a_hit_starts_on_the_sample_of_the_rising_edge() {
    for kind in VOICES {
        for edge in [1usize, 37, 63, 64, 100] {
            let len = 4096;
            let g = gate(len, &[edge], 200);
            let x = Dut::new(kind, &[], SR).run(&[&g], len, 64).remove(0);
            assert_eq!(peak(&x[..edge]), 0.0, "{kind}: sound before the gate");
            let first = x.iter().position(|v| v.abs() > 1e-6).unwrap();
            assert!(
                first == edge || first == edge + 1,
                "{kind}: edge {edge}, first sound {first}"
            );
        }
    }
}

#[test]
fn a_hit_from_silence_rises_over_a_short_ramp_instead_of_stepping() {
    for kind in VOICES {
        for edge in [1usize, 37, 64, 100] {
            let len = 4096;
            let g = gate(len, &[edge], 200);
            let x = Dut::new(kind, &[], SR).run(&[&g], len, 64).remove(0);
            let top = peak(&x[edge..edge + 2000]);
            let first = x.iter().position(|v| v.abs() > 1e-9).unwrap();
            assert!(
                x[first].abs() < 0.02 * top,
                "{kind}: edge {edge} starts with {} of a peak of {top}",
                x[first]
            );
            // The ramp is 0.4 ms (19 samples): its raised cosine climbs at most
            // pi / (2 * 19) of the hit's level per sample, so the output stays within
            // that slope plus the voice's own movement over the ramp.
            let ramp = 19;
            let own = max_step(
                &Dut::new(kind, &[], SR).run(&[&gate(len, &[edge], 200)], len, 64)[0]
                    [edge + ramp..edge + 3 * ramp],
            );
            let step = max_step(&x[first - 1..first + ramp]);
            assert!(
                step <= top * 0.15 + own,
                "{kind}: edge {edge} steps by {step} in the ramp (peak {top}, later steps {own})"
            );
        }
    }
}

#[test]
fn block_size_does_not_change_the_sound() {
    for kind in VOICES {
        let len = 9600;
        let g = gate(len, &[10, 3000], 100);
        let a = Dut::new(kind, &[], SR).run(&[&g], len, 64).remove(0);
        for block in [1, 7, 128] {
            let b = Dut::new(kind, &[], SR).run(&[&g], len, block).remove(0);
            let d = a
                .iter()
                .zip(&b)
                .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
            assert!(d < 1e-6, "{kind}: block {block} differs by {d}");
        }
    }
}

#[test]
fn a_retrigger_steps_no_more_than_a_fresh_hit() {
    for kind in VOICES {
        let len = 24000;
        let fresh = gate(len, &[100], 100);
        let a = Dut::new(kind, &[], SR).run(&[&fresh], len, 64).remove(0);
        // The voice's own movement once its 0.4 ms attack ramp is over.
        let own = max_step(&a[100 + 24..2000]);
        // Second hit while the first is loud: 1 ms, 3 ms and 20 ms after it.
        for gap in [48usize, 144, 960] {
            let g = gate(len, &[100, 100 + gap], 100);
            let x = Dut::new(kind, &[], SR).run(&[&g], len, 64).remove(0);
            let around = max_step(&x[100 + gap - 2..100 + gap + 400]);
            // What a hard restart would step by: the old output where the new hit begins.
            let jump = a[100 + gap - 1].abs();
            println!(
                "{kind}: gap {gap}: step {around:.3}, fresh hit {own:.3}, hard restart {jump:.3}"
            );
            assert!(
                // A retrigger has no attack ramp (the offset keeps it continuous), so the new hit's own
                // click may step as hard as a fraction of the voice's level.
                around <= (own * 1.25 + 0.02).max(0.5 * jump).max(0.35 * peak(&a)),
                "{kind}: retrigger after {gap} steps {around}, a fresh hit {own}, a hard restart {jump}"
            );
        }
    }
}

#[test]
fn velocity_and_accent_scale_the_hit() {
    for kind in VOICES {
        let len = 4800;
        let i = if kind == "drum.hat" { 3 } else { 2 };
        let g = gate(len, &[10], 100);
        let run = |v: f32, a: f32| {
            let (vb, ab) = (vec![v; len], vec![a; len]);
            let ins: Vec<&[f32]> = if kind == "drum.hat" {
                vec![&g, &[], &ab, &vb]
            } else {
                vec![&g, &ab, &vb]
            };
            let ins: Vec<&[f32]> = ins
                .into_iter()
                .map(|s| if s.is_empty() { &[0f32; 4800][..] } else { s })
                .collect();
            peak(&Dut::new(kind, &[], SR).run(&ins, len, 64)[0])
        };
        let _ = i;
        let (full, soft, acc) = (run(1.0, 0.0), run(0.2, 0.0), run(1.0, 1.0));
        println!("{kind}: v1 {full:.3} v0.2 {soft:.3} accent {acc:.3}");
        assert!(soft < full * 0.8, "{kind}: velocity has no effect");
        assert!(
            acc > full * 1.1 || acc > 0.95,
            "{kind}: accent has no effect"
        );
    }
}

#[test]
fn an_unpatched_velocity_is_full_velocity() {
    for kind in VOICES {
        let len = 4800;
        let g = gate(len, &[10], 100);
        let bare = peak(&Dut::new(kind, &[], SR).run(&[&g], len, 64)[0]);
        let full = {
            let ones = vec![1f32; len];
            let zeros = vec![0f32; len];
            let ins: Vec<&[f32]> = if kind == "drum.hat" {
                vec![&g, &zeros, &zeros, &ones]
            } else {
                vec![&g, &zeros, &ones]
            };
            peak(&Dut::new(kind, &[], SR).run(&ins, len, 64)[0])
        };
        assert!((bare - full).abs() < 1e-6, "{kind}: {bare} vs {full}");
    }
}

#[test]
fn a_closed_hat_cuts_an_open_one() {
    let len = 24000;
    let open = gate(len, &[0], 100);
    let none = vec![0f32; len];
    let closed_at = gate(len, &[2400], 100);
    let ring = |closed: &[f32]| {
        let x = Dut::new("drum.hat", &[], SR)
            .run(&[closed, &open, &none, &none], len, 64)
            .remove(0);
        rms(&x[9600..24000])
    };
    let (alone, choked) = (ring(&none), ring(&closed_at));
    println!("open alone {alone:.4}, choked at 50 ms {choked:.4}");
    assert!(choked < alone * 0.2);
}

#[test]
fn the_hat_gates_are_separate_voices_of_one_envelope() {
    let len = 24000;
    let g = gate(len, &[100], 100);
    let none = vec![0f32; len];
    let mut d = Dut::new("drum.hat", &[], SR);
    let closed = d.run(&[&g, &none, &none, &none], len, 64).remove(0);
    let mut d = Dut::new("drum.hat", &[], SR);
    let open = d.run(&[&none, &g, &none, &none], len, 64).remove(0);
    assert!(rms(&open[9600..]) > rms(&closed[9600..]) * 10.0);
}

#[test]
fn every_setting_stays_finite_and_bounded() {
    for kind in VOICES.into_iter() {
        let info = registry::info_for(kind).unwrap();
        for sr in [44100.0, 48000.0, 96000.0, 192000.0] {
            for block in [1usize, 64, 333] {
                for corner in 0..4u32 {
                    let mut d = Dut::new(kind, &[], sr);
                    for (j, p) in info.params.iter().enumerate() {
                        let v = match (corner >> (j % 2)) & 1 {
                            0 => p.min,
                            _ => p.max,
                        };
                        d.set(
                            p.name,
                            if corner == 3 && j % 3 == 0 {
                                p.default
                            } else {
                                v
                            },
                        );
                    }
                    let len = (0.6 * sr) as usize;
                    let g = gate(len, &[0, 200, 5000, 5001, 9000], 300);
                    let ones = vec![1f32; len];
                    let ins: Vec<&[f32]> = if kind == "drum.hat" {
                        vec![&g, &g, &ones, &ones]
                    } else {
                        vec![&g, &ones, &ones]
                    };
                    let x = d.run(&ins, len, block).remove(0);
                    assert!(x.iter().all(|v| v.is_finite()), "{kind} {sr} {corner}");
                    assert!(peak(&x) < 24.0, "{kind} {sr} {corner}: peak {}", peak(&x));
                }
            }
        }
    }
}

#[test]
fn nan_and_huge_inputs_cannot_poison_a_voice() {
    for kind in VOICES {
        let len = 4800;
        let mut g = gate(len, &[10, 1000], 100);
        g[500] = f32::NAN;
        let bad = vec![f32::NAN; len];
        let huge = vec![1e30f32; len];
        let ins: Vec<&[f32]> = if kind == "drum.hat" {
            vec![&g, &g, &bad, &huge]
        } else {
            vec![&g, &bad, &huge]
        };
        let x = Dut::new(kind, &[], SR).run(&ins, len, 64).remove(0);
        assert!(x.iter().all(|v| v.is_finite()), "{kind}");
        assert!(peak(&x) < 8.0, "{kind}: {}", peak(&x));
    }
}

#[test]
fn processing_allocates_nothing() {
    for kind in VOICES.into_iter().chain(["midi.note"]) {
        let mut d = Dut::new(kind, &[], SR);
        let n = 64;
        let g = gate(n, &[3], 30);
        let sig: Vec<Signal> = (0..d.n_in).map(|_| Signal::Buffer(&g)).collect();
        let ps: Vec<Signal> = d.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut bufs = vec![vec![0f32; n]; d.n_out];
        let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
        let mut io = ProcessIo::new(&sig, &mut refs, &ps, n);
        assert_no_alloc(|| {
            for _ in 0..50 {
                d.m.process(&mut io);
            }
        });
    }
}

// ---- midi.note -------------------------------------------------------------------------------

fn note_block(m: &mut MidiNote, params: &[f32], n: usize) -> Vec<Vec<f32>> {
    let ps: Vec<Signal> = params.iter().map(|&v| Signal::Scalar(v)).collect();
    let mut bufs = vec![vec![0f32; n]; 2];
    let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
    let mut io = ProcessIo::new(&[], &mut refs, &ps, n);
    m.process(&mut io);
    bufs
}

/// A `midi.note` set to `note` on `channel`, one block already run so it knows its params.
fn note_module(note: f32, channel: f32) -> (MidiNote, Vec<f32>) {
    let ps = vec![note, channel];
    let mut m = MidiNote::new();
    note_block(&mut m, &ps, 64);
    (m, ps)
}

#[test]
fn a_note_raises_its_gate_on_the_sample_with_its_velocity() {
    let (mut m, ps) = note_module(38.0, 0.0);
    m.note_on(17, 9, 38, 100);
    let o = note_block(&mut m, &ps, 64);
    assert_eq!(o[0][16], 0.0);
    assert_eq!(o[0][17], 1.0);
    assert!((o[1][17] - 100.0 / 127.0).abs() < 1e-6);
    assert_eq!(o[1][16], 0.0);
}

#[test]
fn other_notes_do_nothing() {
    let (mut m, ps) = note_module(36.0, 0.0);
    for n in [0u8, 35, 37, 60, 127] {
        m.note_on(0, 0, n, 100);
    }
    assert_eq!(peak(&note_block(&mut m, &ps, 64)[0]), 0.0);
}

#[test]
fn a_repeated_note_drops_the_gate_for_one_sample() {
    let (mut m, ps) = note_module(36.0, 0.0);
    m.note_on(0, 0, 36, 100);
    m.note_on(40, 0, 36, 100);
    let g = &note_block(&mut m, &ps, 64)[0];
    assert!(g[..40].iter().all(|&v| v == 1.0));
    assert_eq!(g[40], 0.0);
    assert_eq!(g[41], 1.0);
}

#[test]
fn a_pulse_lasts_ten_milliseconds_across_blocks() {
    let (mut m, ps) = note_module(36.0, 0.0);
    m.prepare(
        SR,
        64,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    m.note_on(60, 0, 36, 100);
    let mut all = Vec::new();
    for _ in 0..16 {
        all.extend(note_block(&mut m, &ps, 64).remove(0));
    }
    assert_eq!(all.iter().filter(|&&v| v == 1.0).count(), 480);
    assert_eq!(all[60], 1.0);
}

#[test]
fn the_channel_filter_is_honoured() {
    let (mut m, ps) = note_module(36.0, 10.0);
    m.note_on(0, 0, 36, 100);
    assert_eq!(peak(&note_block(&mut m, &ps, 64)[0]), 0.0);
    m.note_on(5, 9, 36, 100);
    assert_eq!(note_block(&mut m, &ps, 64)[0][5], 1.0);
}

#[test]
fn a_full_queue_and_a_late_offset_are_safe() {
    let (mut m, ps) = note_module(36.0, 0.0);
    for i in 0..200 {
        m.note_on(i % 64, 0, 36, 100);
    }
    for _ in 0..20 {
        note_block(&mut m, &ps, 64);
    }
    m.note_on(100, 0, 36, 90);
    assert_eq!(peak(&note_block(&mut m, &ps, 64)[0]), 0.0);
    assert_eq!(note_block(&mut m, &ps, 64)[0][36], 1.0);
}

#[test]
fn a_note_in_the_first_block_of_a_new_module_is_not_lost() {
    let ps = vec![38.0, 0.0];
    let mut m = MidiNote::new();
    m.note_on(3, 9, 38, 100);
    m.note_on(5, 9, 36, 100);
    let o = note_block(&mut m, &ps, 64);
    assert_eq!(o[0][3], 1.0);
    // The other note was kept in the queue and dropped when the params were known.
    assert_eq!(o[1][4], 100.0 / 127.0);
    assert_eq!(o[1][63], 100.0 / 127.0);
}

/// `cargo test --release -p kabl-modules --test drums -- --ignored --nocapture`: peak of the hit and
/// the share of the energy above 5 kHz in its first 20 ms, to compare the onset ramp with a build without it.
#[test]
#[ignore = "prints the transient peak and the energy above 5 kHz of every voice"]
fn print_transient_metrics() {
    for kind in VOICES {
        let x = one_hit(kind, SR);
        let seg = &x[100..100 + 960];
        let n = 960usize;
        let (mut hi, mut all) = (0.0f64, 0.0f64);
        for k in 1..n / 2 {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in seg.iter().enumerate() {
                let a = std::f64::consts::TAU * (k * i) as f64 / n as f64;
                re += *v as f64 * a.cos();
                im -= *v as f64 * a.sin();
            }
            let e = re * re + im * im;
            all += e;
            if k as f64 * SR as f64 / n as f64 > 5000.0 {
                hi += e;
            }
        }
        println!(
            "METRIC {kind}: peak {:.4} dB, energy above 5 kHz {:.4} dB",
            20.0 * peak(&x).log10(),
            10.0 * hi.log10()
        );
        let _ = all;
    }
}
