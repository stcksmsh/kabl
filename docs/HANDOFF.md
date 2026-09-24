# Handoff — for the next blank-context agent

Read this, then `docs/STATUS.md` (top section) and the last entries of `docs/decisions.md`.
Trust code and git over older docs.

## Where things are

- Branch `master`, pushed to `origin` on 2026-09-24 at Kosta's request (sound palette WIP). The rack UI migration is **closed**: Kosta
  reviewed it hands-on and approved it (decisions.md, "Rack migration closed"). Record:
  `docs/rack-migration/README.md` (screenshots, `walkthrough.mp4`, verification).
- Workflow: commit small working changes straight to `master`; no branches, PRs or pushes
  unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use. Don't `cargo fmt` the whole
  crate: it reformats the untouched `rev2_proto` / `rev2_env_ab` examples; use `rustfmt` on
  the files you changed.

Launch (real audio + MIDI). The newest demo is `--patch patches/composition --perform --rate 48000 --frames 256`
(checklist: `docs/composition-batch/CHECKLIST.md`):

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
- Verification: `cargo test --workspace` 296 pass (9 ignored = fixture/patch/clip writers), clippy clean.
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

## Performance batch — closed (Kosta approved hands-on, 2026-09-24)

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

Follow-up (same day, `3c41a55..`): labels (`Op::SetLabel`, schema v3), compact panel,
`gain`, output meter (`record::PeakTap`), MIDI reconnect/notes-off, CC undo groups
(`PatchLog::append_to_group`), recorder never-overwrite and failure handling, single-instance
audit (`compile_per_voice`, `tests/single_instance.rs`), RT priority + callback telemetry
(`main.rs` `CallbackTiming`, `KABL_STATS_FILE`), soak. Scripts in
`docs/performance-batch/scripts/` (`recovery.txt`, `load.txt`, `make_soak.py`); drive.py has
`midikill`/`midistart`. Test hooks: `KABL_RECORD_FAIL_AFTER`, `KABL_RECORD_RING_FRAMES`,
`--no-rt`. Kosta approved the whole batch hands-on. Next scope goes through the supervisor.

## Composition + Motion — built, waiting for Kosta's hands-on review

Supervisor scope, one batch. Record, evidence and checklist: `docs/composition-batch/README.md`
(+ `CHECKLIST.md`, `design.md` for the launch rules); rationale: decisions.md "Composition +
Motion batch". Commits `5d54b68..` on master.

- `seq.rs`: banks A–D (A = the historic param names, B–D prefixed `b.`/`c.`/`d.`; helpers
  `bank_of`, `bank_param`, `slot_name`), `direction`, probability `r1..r8`, startup `bank`,
  `arm`/`cancel`, seeded xorshift; state rides in `carry_from`.
- Engine: `Clock` counts pulses per epoch (`next_tick`, `block_ticks`); `PatchEngine` holds
  pending launches (`launch`, `cancel`, `command(&Command)`, `seqs` report) and arms
  sequencers inside `CompiledPatch::process_block_with`; clocks are scheduled first.
  `MAX_PARAMS` 143, `MAX_OUTPUTS` 4, per-step params prebuilt.
- New modules: `macro` (`macros.rs`), `cues` (`cues.rs`, no audio). `lfo` gained clock sync.
- UI: `banks.rs` (bank ops, launch settings), `cues.rs` (cue data, `references_to` used by
  `PatchEditor::remove_module`), `perform.rs` (bank/cue cards, `btn.*` MIDI buttons),
  `rack.rs` (`View::edit_banks`, `visible`, `face_name`), `lib.rs` (bank strip, cue face,
  drawer panels). `main.rs`: command queue, seq/LFO reports, stereo config with `--rate`,
  worst-callback time in the stats line.
- Demo `patches/composition`, built by `crates/ui/tests/composition.rs`
  (`cargo test -p kabl-ui --test composition write_composition_patch -- --ignored`).
  Scripts: `docs/composition-batch/scripts/` (take, walkthrough, shots);
  `record-walkthrough.sh`; `examples/bench_composition`. `drive.py` takes `KABL_DRIVE_LOG`.
- Pitfall: a scripted Cancel right after a launch races the bar (a bar is 2.1 s at 112 bpm);
  send a Restart first (button CC 47) to start the bar grid, as the walkthrough does.

## Sound Palette + Playable Voices — IN PROGRESS (stages A–B partly done)

Supervisor scope, one batch (full brief: the owner's prompt of 2026-09-24; summary below).
Composition + Motion is still **waiting for Kosta's hands-on review** — keep the two separate,
approve neither on his behalf.

Goal: distinct playable strings, pads, leads, basses, textures, percussion. Stages: A osc/noise/
filter, B chorus/drive/keyboard, C patch library + `patches/sound-palette` + 8–10 min take +
isolated examples/comparisons + walkthrough, D regression, callback measurements (execution,
arrival, xruns separately; carry the Composition 3–5 ms spike watch item), screenshots at
1440×900 and 1280×800 in both themes, docs in `docs/sound-palette-batch/` (README with
sources/licenses/limits, launch commands, patch guide, CHECKLIST), HANDOFF/STATUS/decisions.

### Done and committed

- `ffdbd26` **osc.va**: `pw` (5–95 %, per-sample ramp, DC removed), `fine` (±100 ct), `unison`
  1–4 (1/√N, 20 ms fades, fixed spread table), `detune`; band-limited hard sync (interpolated
  edge, PolyBLEP residual, in-block lookahead). Defaults bit-exact with the old oscillator (legacy
  render tests pass). New params are advanced. Tests + alias measurements:
  `crates/modules/tests/osc_palette.rs` (`-- --nocapture` prints the table). Measured: saw
  worst alias −49 dB @440, −37 @1760, −30 @3520 (naive −38/−26/−20); sync 7–10 dB better
  than the old naive sync. PolyBLEP limits at high notes are documented, not hidden.
- `a89a568` **noise** (white/pink Kellet, both −14 dBFS RMS, seeded by module id + lane in
  `compile.rs`, `carry_from` only for the same seed) and **filter.ladder** (Zavalishin ZDF
  4-pole, tanh input stage, k = 4.4·res, input compensation 1+k/2, drive 0–24 dB, cutoff_cv
  2^cv, g ramped per block). Tests: `tests/noise.rs`, `tests/ladder.rs`.
- `5c0c1c4` **chorus** (stereo, one swept tap per side — 3-tap version notched the wet by
  10 dB, see module doc; equal-power mix, exact endpoints; width = right-side phase offset) and
  **drive** (tanh(g·x)/tanh(g), g = 10^(dB/20)−1, first-order ADAA, 0.5-sample latency,
  exact bypass at 0 dB with 10 ms engage fade; no oversampling — measured 8–13 dB alias
  reduction, report it honestly). Tests: `tests/chorus_drive.rs`. Test helpers (Rig, FFT,
  alias_db): `crates/modules/tests/common/mod.rs`.

### Committed as work in progress (this commit): keyboard expression

- Spec first: `docs/sound-palette-batch/keyboard.md` (the rules; implement to it).
- `crates/engine/src/keyboard.rs`: `KeyEvent` (incl. `from_midi`: notes, CC64, CC120/123),
  `Keyboard` (POLY LRU/steal, pedal, MONO/LEGATO, priorities, glide flags), `Action`.
- `midi.in` params `mode`/`priority`/`glide`/`glide_ms`; `play(pitch, vel, glide,
  retrigger)` with a one-sample gate dip; `configure()` called from `compile.rs`.
- `CompiledPatch::keyboards`, `key_action`; `PatchEngine::key`, `keys()`, `sync()` (per
  `midi.in` keyboards, mode change / Load / deleted module release everything).
- `crates/engine/tests/keyboard.rs`: 19 pass.

### Next steps (in order)

1. **Failing test**: `kabl-ui` `rack::tests::controls_stay_inside_their_panel_and_apart` —
   `midi.in` now has params; its column layout (`rack.rs` ~l.422, Keys decor) overlaps the
   new selectors with its jacks. Fix the layout (widen midi.in or place selectors below the
   keys decor), add step labels in `routing.rs::step_labels` (`mode` POLY/MONO/LEGATO,
   `priority` LAST/LOW/HIGH, `glide` OFF/ALWAYS/LEGATO). All else: 380+ pass, clippy clean.
2. **Wire MIDI in `crates/ui/src/main.rs`**: replace `VoiceEvent`/`VoiceAllocator` in
   `MidiSink` with `KeyEvent::from_midi` → rtrb → `engine.key(e)` on the audio thread; keep CC
   64/120/123 out of the learn queue; `release_all` = push `KeyEvent::AllOff`; stats line
   "MIDI notes held" from `engine.keys()` via an atomic. Leave `kabl-standalone` as is.
3. UI: short names exist for new kinds (`routing.rs`); check faces/advanced areas for osc,
   ladder, chorus, drive, noise, midi.in in the real app (both sizes, both themes).
4. Engine test `crates/engine/tests/sound_palette.rs`: self-swap null test with all new
   modules, overlapping swaps, no allocation; noise per-voice independence and
   single-instance (lane-0 stream) documented behaviour.
5. Stage C patches (build via a `crates/ui/tests/<name>.rs` writer like `composition.rs`),
   measured headroom incl. chords; `patches/sound-palette`; recordings via
   `docs/composition-batch/scripts` + `drive.py` (`KABL_PLAYER=1`); label scripted takes.
6. Stage D docs and measurements as above. Then stop for Kosta's hands-on review.

## Accepted limitations

Block-rate modulation; no hysteresis on stepped destinations; pitch full scale ±60 st; state
carry O(modules²) per swap; one cable per jack input; quiet engine output; Pi 4 unmeasured;
drawer sliders lack Shift/Escape; selector routes show no reachable-options bracket; lanes pan
into view only on inspection; no `labels_on_art` contrast warning; no built-in skin;
`docs/modulation-slice/xdotool-walkthrough.sh` is stale for the rack (use `drive.py`).

## Not in scope

New synthesis, full rich LFO, plugin work, compact layout, framework change, MIDI sync,
probability/swing, tape modeling.
