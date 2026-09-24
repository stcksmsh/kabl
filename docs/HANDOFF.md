# Handoff — for the next blank-context agent

Read this, then `docs/STATUS.md` (top section) and the last entries of `docs/decisions.md`.
Trust code and git over older docs.

## Where things are

- Branch `master`, local only (not pushed). The rack UI migration is **closed**: Kosta
  reviewed it hands-on and approved it (decisions.md, "Rack migration closed"). Record:
  `docs/rack-migration/README.md` (screenshots, `walkthrough.mp4`, verification).
- Workflow: commit small working changes straight to `master`; no branches, PRs or pushes
  unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use. Don't `cargo fmt` the whole
  crate: it reformats the untouched `rev2_proto` / `rev2_env_ab` examples; use `rustfmt` on
  the files you changed.

Launch (real audio + MIDI). The newest demo is `--patch patches/performance --perform`:

    cargo run --release -p kabl-ui -- --patch patches/reference            # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/reference --size 1280x800
    cargo run --release -p kabl-ui -- --patch patches/crowded

## What exists (owner-approved)

- Production rack (`crates/ui`): `rack.rs` pure layout with tests, `theme.rs`, `lib.rs`
  rendering/toolbar/pan/zoom/drawer, `routing.rs` modulation. A-light / A-dark, All/Focus/
  Hidden cables, user-chosen faces (`face.<param>` params, no audio rebuild), push/float
  expansion, 50–200 % zoom, pull-the-plug cable moves, skin renderer (no built-in skin).
- Modulation: any knob takes routes (signed amount, invert, bypass); ring/lanes/drawer are one
  route; taper-space sum, one clamp, block rate; envelope timing CONT/KEY.
- Verification: `cargo test --workspace` 279 pass (9 ignored = fixture/patch/clip writers), clippy clean.
  Real-X tooling: `docs/rack-migration/drive.py` (+ `scripts/`), `closeout-real-x.sh`,
  `record-walkthrough.sh`; `docs/interlocking-sequences/record-walkthrough.sh` records a patch
  without MIDI.

## Sequencing — closed (Kosta approved the walkthrough, af0585b)

`clock` (BPM, 16th-note gate) and the 8-step `seq` module (pitch plus OFF/ON gate per step,
length, reset input, and a step light fed by the first audio → UI queue) are both global rate. The demo is `cargo run --release -p kabl-ui -- --patch patches/sequence`.
The `decisions.md` entry "Shared clock + basic pitch/gate sequencing" lists what was built and
what was deliberately left out.

## Interlocking sequences — closed (Kosta approved hands-on, b7a645b)

Supervisor scope: two interlocking sequences from one shared clock. Built: clock transport
(`Clock::command`, `Transport::{Run, Stop, Restart}`, sent UI → audio through an `rtrb` queue in
`crates/ui/src/main.rs`, never in the op log), the clock's `reset` output, `clock.div`, and
`seq.transpose`. Module panel sliders now use the knob's formatter. The demo is
`cargo run --release -p kabl-ui -- --patch patches/interlocking` (built by
`crates/ui/tests/interlocking.rs`; rewrite it with `-- --ignored`). Record:
`docs/interlocking-sequences/README.md` (video, screenshots, verification, deferred items).
Deferred, not blockers: toolbar transport copy; Fit density of the four-row demo. Next scope
goes through the supervisor.

## Echo — closed (Kosta approved hands-on, 2026-09-24)

`delay` (global): mono in, stereo out, clock-synced or free time, feedback, mix, tone in the
loop, MONO/PING. `Module::carry_from` carries its 4 s lines across live swaps (audio thread,
no allocation). Load now builds a `fresh` graph that carries nothing. The demo is
`cargo run --release -p kabl-ui -- --patch patches/echo` (built by `crates/ui/tests/echo.rs`;
rewrite it with `cargo test -p kabl-ui --test echo write_echo_patch -- --ignored`, keeping the
name filter, because the file includes `interlocking.rs` and its writer). Record, video, clips, timings and the listening checklist:
`docs/echo/README.md`. Timing harness: `crates/ui/examples/bench_echo.rs`. Next scope goes
through the supervisor; don't extend effects (reverb, tape, etc.) without it.

## Performance batch — built, waiting for Kosta (2026-09-24)

Supervisor scope, authorized as one batch. Record and checklist:
`docs/performance-batch/README.md`; rationale: decisions.md "Performance batch".

- `reverb` (Dattorro plate, `crates/modules/src/builtins/reverb.rs`), `seq` velocity + gate
  length (`seq.rs`; CLOCK stays the default), `crates/ui/src/perform.rs` (pins `pin.*`, CC
  maps `cc.*`, soft takeover, panel), `record.rs` (recorder), `main.rs` (MIDI input switching,
  CC queue, `--rate/--frames/--record-dir/--perform/--midi`, callback timing).
- Engine: voice-rate modules no `midi.in` reaches compile to one instance (`compile.rs`,
  `voiced`), with a carry fallback between the two.
- Demo `patches/performance`, built by `crates/ui/tests/performance.rs`
  (`cargo test -p kabl-ui --test performance write_performance_patch -- --ignored`).
- Scripted real-app runs: `drive.py` now takes `KABL_ARGS`, `KABL_PLAYER=1` (starts
  `examples/midi_player`, a virtual MIDI port) and `midi …` lines;
  `docs/performance-batch/record-walkthrough.sh`; the long take's script comes from
  `scripts/make_performance.py`. Timing: `examples/bench_performance`.
- Pitfalls: a rack target hidden under the Perform panel still has a hit rect, so drive.py
  clicks the panel instead (and a following ctrl+z can undo the patch's own pins, which are
  the last op of its log). Close the panel or pan first. The panel's horizontal scroll hides
  far cards; new/moved pins scroll into view, transport stays at the left.

## Accepted limitations

Block-rate modulation; no hysteresis on stepped destinations; pitch full scale ±60 st; state
carry O(modules²) per swap; one cable per jack input; quiet engine output; Pi 4 unmeasured;
drawer sliders lack Shift/Escape; selector routes show no reachable-options bracket; lanes pan
into view only on inspection; no `labels_on_art` contrast warning; no built-in skin;
`docs/modulation-slice/xdotool-walkthrough.sh` is stale for the rack (use `drive.py`).

## Not in scope

New synthesis, full rich LFO, plugin work, compact layout, framework change, MIDI sync,
probability/swing, tape modeling.
