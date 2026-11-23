//! The 1.0 promo patch (docs/promo/STORYBOARD-V1.md): Lichen with Balcony's chords and
//! arpeggio on Lichen's clock, re-voiced from C major into D dorian, brought in by a sixth
//! macro, **Chords**, and a cue, **Bloom+** (Bloom with Focus and Chords at 1).
//!
//! ```sh
//! cargo run --release -p kabl-tools --example presets_piece -- promo write docs/promo/lichen-plus.kabl docs/promo/empty.kabl
//! cargo run --release -p kabl-tools --example presets_piece -- promo render OUT.wav TIMELINE.json
//! ```
//!
//! `render` plays take B of the storyboard the way the recording drives it: 8 bars of silence
//! (the take's bar 0 is a click on the Rack tab, the clock runs at bar 8), then the cues on the
//! storyboard's bars. The wav is the reference `docs/promo-demo/scripts/post.py` matches the
//! recording against; the timeline is its bar length (no captions).

use super::*;

const BPM: f32 = 112.0;
/// The bar (storyboard numbering) the clock starts on in take B.
const RUN_BAR: f32 = 8.0;
/// The bar the video ends on.
const END_BAR: f32 = 40.0;

/// Lichen's voices plus the layer.
fn voices(r: &mut Rack, clock: ModuleId) -> Vec<(ModuleId, &'static str)> {
    let mut v = lichen_voices(r, clock);
    v.push(layer(r, clock));
    v
}

/// Balcony's three chord voices and its triangle arpeggio, one chord a bar: Dm, F/C, C, G/B
/// (i, III, VII, IV of D dorian, the same notes as Balcony's C major). Silent until Chords
/// opens its level; its own room makes it the soft, wide part over Lichen's loops.
fn layer(r: &mut Rack, clock: ModuleId) -> (ModuleId, &'static str) {
    let knob =
        *r.e.state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .expect("the macros come first")
            .0;
    let chord = |steps: &'static str| Line {
        wave: 2.0,
        uni: 3.0,
        det: 10.0,
        gate: 90.0,
        env: [20.0, 400.0, 0.6, 300.0],
        lp: 2500.0,
        db: -8.0,
        div: 16.0,
        ..Line::new(steps)
    };
    let lines = [
        chord("2 0 0 -1"),
        chord("5 5 4 2"),
        chord("9 9 7 7"),
        Line {
            wave: 1.0,
            oct: 1.0,
            gate: 50.0,
            env: [1.0, 300.0, 0.0, 100.0],
            db: -12.0,
            lp: 6000.0,
            div: 2.0,
            ..Line::new("2 5 9 14 9 5 9 5 0 5 9 12 9 5 9 5 0 4 7 12 7 4 7 4 -1 2 7 14 7 2 7 2")
        },
    ];
    let outs: Vec<(ModuleId, &str)> = lines.iter().map(|l| add_line(r, clock, l).out).collect();
    let mix = sum(r, &outs);
    let level = r.add("gain", &[]);
    r.wire(mix, "out", level, "in");
    let min = param_of("gain", "gain_db").min;
    rise(r, knob, 6, level, "gain", "gain_db", min, -2.0);
    let room = r.add(
        "reverb",
        &[
            ("decay_s", 3.5),
            ("damp_hz", 4500.0),
            ("mix", 40.0),
            ("predelay_ms", 30.0),
            ("width", 80.0),
        ],
    );
    r.wire(level, "out", room, "in");
    let split = r.split(room);
    let mono = r.add("mixer", &[("level1", 0.5), ("level2", 0.5)]);
    r.wire(split, "left", mono, "in1");
    r.wire(split, "right", mono, "in2");
    (mono, "out")
}

/// The patch and its cues in pad order: Spore, Colony, Bloom, Bloom+, Die-back. `bar` is the
/// storyboard bar each cue lands on.
fn build() -> (Rack, Vec<CueDef>) {
    let (mut r, mut defs) = lichen_with(voices);
    let of = |r: &Rack, kind: &str| {
        *r.e.state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == kind)
            .expect(kind)
            .0
    };
    let (k, cue) = (of(&r, "macro"), of(&r, "cues"));
    r.e.set_label(k, "name.m6", Some("Chords".into()));
    r.e.set_param(k, "m6", 0.0);
    for n in 1..=4 {
        cues::set_macro(&mut r.e, cue, n, k, 5, Some(0.0));
    }
    let n = cues::add(&mut r.e, cue, &|_| 0).expect("cue");
    cues::rename(&mut r.e, cue, n, "Bloom+");
    for (m, v) in [0.8, 0.9, 0.9, 0.6, 1.0, 1.0].into_iter().enumerate() {
        cues::set_macro(&mut r.e, cue, n, k, m as u8, Some(v));
    }
    cues::set_morph(&mut r.e, cue, n, 8.0);
    cues::reorder(&mut r.e, cue, n, 4);
    r.e.set_presentation(&[
        (k, format!("{PIN_PREFIX}m6"), Some(7.0)),
        (k, format!("{CC_PREFIX}m6"), Some(25.0)),
        (cue, format!("{BTN_PREFIX}cue.4"), Some(43.0)),
        (cue, format!("{BTN_PREFIX}cue.5"), Some(44.0)),
    ]);
    let die_back = defs.pop().expect("Die-back");
    defs.push(CueDef {
        name: "Bloom+",
        targets: [0.8, 0.9, 0.9, 0.6],
        morph: 8.0,
        bar: 0.0,
    });
    defs.push(die_back);
    for (d, bar) in defs.iter_mut().zip([8.0, 16.0, 24.0, 26.0, 36.0]) {
        d.bar = bar - RUN_BAR;
    }
    (r, defs)
}

pub(super) fn main(args: &[String]) {
    let (rack, defs) = build();
    match args.first().map(String::as_str) {
        Some("write") => {
            kabl_core::format::save_kabl(Path::new(&args[1]), rack.e.log()).expect("save");
            println!("wrote {}", args[1]);
            // The drive script's ids: the step-pattern cables (the 7- and 5-loops).
            for (id, c) in &rack.e.state().cables {
                if let Some(len) = c.params.get("length") {
                    println!("pattern cable {id:?}: length {len} into {:?}", c.to);
                }
            }
            // Take A starts from an empty rack: a saved patch with no modules.
            if let Some(seed) = args.get(2) {
                kabl_core::format::save_kabl(Path::new(seed), PatchEditor::new().log())
                    .expect("save");
                println!("wrote {seed}");
            }
        }
        Some("render") => {
            let bar = 240.0 / BPM;
            let secs = 0.5 + (END_BAR - RUN_BAR) * bar + 1.0;
            let (l, r) = render(rack.e.state(), &[], secs, Some((&defs, BPM)));
            // The take's bar 0 is RUN_BAR bars before the clock starts (render runs it at 0.5 s).
            let lead = vec![0.0; (RUN_BAR * bar * sr()) as usize];
            let (l, r) = ([&lead[..], &l].concat(), [&lead[..], &r].concat());
            write_wav(Path::new(&args[1]), &l, &r);
            let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
            println!("peak {:.2} dBFS", 20.0 * peak.max(1e-9).log10());
            for (name, from, to) in [
                ("spore", 8.0, 16.0),
                ("colony", 16.0, 24.0),
                ("bloom", 24.0, 26.0),
                ("bloom+", 28.0, 36.0),
                ("die-back", 36.0, 40.0),
            ] {
                let a = ((0.5 + from * bar) * sr()) as usize;
                let b = (((0.5 + to * bar) * sr()) as usize).min(l.len());
                let p = l[a..b].iter().chain(&r[a..b]).fold(0.0f32, |m, s| m.max(s.abs()));
                let ms = l[a..b].iter().chain(&r[a..b]).map(|s| s * s).sum::<f32>()
                    / (2 * (b - a)) as f32;
                println!(
                    "  bars {from}-{to} {name:9} peak {:6.2} dBFS, rms {:6.2} dBFS",
                    20.0 * p.max(1e-9).log10(),
                    10.0 * ms.max(1e-12).log10()
                );
            }
            std::fs::write(
                &args[2],
                format!(
                    "{{\"bar_seconds\": {bar}, \"bars\": {END_BAR}, \"sections\": [], \"gestures\": []}}\n"
                ),
            )
            .expect("timeline");
        }
        _ => eprintln!("usage: presets_piece promo write PATCH.kabl [EMPTY.kabl] | render OUT.wav TIMELINE.json"),
    }
}
