# Handoff — for the next blank-context agent

Read this, then `docs/STATUS.md` (top section) and the last entries of `docs/decisions.md`.
Trust code and git over older docs.

## Product direction and continuation planning — 2026-09-25

Kosta requested a product/code audit, competitor research and a sequential plan for fresh
implementing agents. See `docs/PRODUCT-PLAN.md`, `docs/product-research/RESEARCH.md` and
`docs/product-research/AGENT-WORKFLOW.md`. The proposed first brief is
`docs/product-research/briefs/D01.md`. **These are planning documents, not authorization
of all proposed batches.** Composition + Motion and Sound Palette still await separate
owner reviews; no approval status changed in this planning session.

Confirmed priorities: shape useful sounds quickly; musicians new to modular; Linux/REAPER
first; laptop-class performance (Pi was an example, not a target); eventual reusable
composites, custom light/dark panels and actual user-written DSP/behaviour. See the latest
decisions entry. Proposed ordering/architecture remain recommendations pending disposition.

## D01-R1 saving repairs — submitted (2026-09-25)

Owner-authorized repair, run in parallel with D02 (its own branch/PR; D01-R1 merges first).
Both supervisor concerns reproduced and fixed; the real window-manager close is verified in
the cloud (Xvfb + openbox). Record: `docs/find-play-save/repair-1/REPORT.md` (+ `REVIEW.md`,
fresh reviewer subagent, final recheck at b579fb3). Code: `library.rs` `save_folder` /
`repair_folder` (staging in `<folder>/.kabl-save` with a `kabl-staging` marker; `.kabl-save`
is reserved), `ReplaceMismatch`, fault hook `KABL_SAVE_FAIL` / `library::fail_saves_at`;
`browser.rs` Save As offer tied to its name. drive.py: `wmclose`, `running`, `exited`
(needs an EWMH WM: `repair-1/scripts/openbox-rc.xml`). D01's owner checklist, Composition +
Motion and Sound Palette reviews remain pending. Preview picker/Stop semantics unchanged.

## D01 Find, play and save — built, awaiting Kosta's review (2026-09-25)

Kosta authorized D01 as a whole and **explicitly deferred** the Composition + Motion and Sound
Palette hands-on reviews while his laptop is away. Those two stay **pending, not approved**;
D00 is not complete; no later batch (D02+) is authorized. Record, evidence, limits:
`docs/find-play-save/README.md` (+ `CHECKLIST.md`, `REPORT.md`); rationale: decisions.md
"D01 Find, play and save a sound".

- Built in a cloud session on branch `claude/install-caveman-ponytail-bhigco` (the
  container required it); PR https://github.com/stcksmsh/kabl/pull/2 to `master` is open for Kosta to merge.
- Code: `crates/ui/src/library.rs` (discovery, `sound.toml` metadata, staged user saves and
  repair, favorites/recents), `crates/ui/src/browser.rs` (panel, document/unsaved state,
  audition, Start/Stop, dialogs), `crates/engine/src/patch_engine.rs` (`Command::Preview`,
  `PreviewStop`, per-source key ownership), `main.rs` (library at start, pieces open
  stopped, quit guard). Tests: `crates/engine/tests/preview.rs`, `crates/ui/tests/{library,browser}.rs`.
- Factory metadata: `patches/**/sound.toml` and the new `patches/init-keyboard`, written by
  `cargo test -p kabl-ui --test library write_factory_library -- --ignored`.
- Package: `packaging/linux/package.sh` → `target/dist/kabl-<v>-linux-<arch>.tar.gz`
  (`bin/kabl-ui`, `share/kabl/patches`). User data: `~/.local/share/kabl/`.
- Scripted real-app tooling: `drive.py` now takes `KABL_BIN`, patch `-` (no `--patch`) and
  `fill KEY TEXT`; its `save DIR` uses the browser's advanced folder section. D01 scripts in
  `docs/find-play-save/scripts/`; `record-walkthrough.sh`. Hit keys: `sound:<id>`, `open`,
  `play`, `stop-preview`, `start`, `stop`, `save`, `save-as`, `dlg:*`, `filter:*`, `fav:<id>`.
- Open items for the review: the 60-second first-sound observation, controller/preview feel,
  laptop timing at 256 frames. Composition spike and Sound Palette stall questions unchanged.

## Where things are

- Branch `master` on `origin`. The sound palette batch was merged into it on 2026-09-25 (PR #1,
  merge 73be119). Kosta allows pushing straight to `master` (solo, unreleased project). The
  rack UI migration is **closed**: Kosta
  reviewed it hands-on and approved it (decisions.md, "Rack migration closed"). Record:
  `docs/rack-migration/README.md` (screenshots, `walkthrough.mp4`, verification).
- Workflow: commit small working changes straight to `master` and push them; no branches or
  PRs unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use. Don't `cargo fmt` the whole
  crate: it reformats the untouched `rev2_proto` / `rev2_env_ab` examples; use `rustfmt` on
  the files you changed.

Launch (real audio + MIDI). The newest demo is `--patch patches/sound-palette --perform --rate 48000 --frames 256`
(checklist: `docs/sound-palette-batch/CHECKLIST.md`); Composition's is `--patch patches/composition`:

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
- Verification: `cargo test --workspace` 407 pass (13 ignored = fixture/patch/clip/render writers), clippy clean.
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

## Sound Palette + Playable Voices — built, waiting for Kosta's hands-on review

Supervisor scope, one batch. Record, evidence, measurements, limits:
`docs/sound-palette-batch/README.md` (+ `CHECKLIST.md`, `PATCHES.md`, `keyboard.md` for the
keyboard rules); rationale: decisions.md "Sound palette batch". Composition + Motion is
still **separately waiting for its review** — approve neither on Kosta's behalf.

**Where the commits are:** finished in a cloud session on the branch
`claude/sound-palette-playable-voices-q6j9ft` (c41420c..04749eb), merged into `master` as PR #1
(73be119) on 2026-09-25. Merged is not approved: the hands-on review is still open. 

- Modules (`crates/modules/src/builtins/`): `osc_va` (pw, fine, unison, detune, band-limited
  sync), `noise`, `filter_ladder`, `chorus`, `drive`. Tests in `crates/modules/tests/`
  (`osc_palette`, `noise`, `ladder`, `chorus_drive`; `-- --nocapture` prints the alias tables).
- Keyboard: `crates/engine/src/keyboard.rs`, `PatchEngine::key/keys`, `midi.in` params
  mode/priority/glide/glide_ms; `crates/engine/tests/keyboard.rs`. The app's MIDI goes
  through `KeyEvent::from_midi` → rtrb → `engine.key()` (`crates/ui/src/main.rs`); CC
  64/120/123 never reach learn; notes-off/disconnect push `AllOff`.
- `compile.rs`: a noise chain whose every consumer is voiced runs per voice.
- UI: `rack.rs` `midi_in_face` (keys + settings line, full-width selectors, jacks row,
  overflow to the advanced area); `routing.rs` step labels; `lib.rs` `midi_in_summary`.
- Patches: `patches/palette/{strings,pad,lead,bass,breath,perc}`, `patches/sound-palette`,
  built by `crates/ui/tests/sound_palette.rs`
  (`cargo test -p kabl-ui --test sound_palette write_palette_patches -- --ignored`; also
  `write_comparisons`, `write_offline_renders`; `headroom_of_every_patch -- --nocapture`).
- Engine tests: `crates/engine/tests/sound_palette.rs` (self-swap nulls, histories, noise,
  lifecycle). Timing: `examples/bench_palette`, `examples/bench_palette_parts`.
- Scripts: `docs/sound-palette-batch/scripts/` (`make_piece.py` → `piece.txt`,
  `make_examples.py` → `examples/`, `walkthrough.txt`, `keys.txt`, `faces-*.txt`,
  `shots-piece.txt`, `watch_stats.py`), `record-walkthrough.sh`.
- Container test hooks: `KABL_MIDI_PIPE=/path` (fifo stand-in MIDI port `kabl-pipe`; the
  midi_player writes it; drive.py `midikill` removes it). drive.py gained `wait T`,
  `goto KEY X Y`, `KABL_APP_LOG`. Without rtkit, give the audio thread RT with
  `chrt -f -p 80 <tid of cpal_alsa_out>`.
- Open items for the review: the dense piece's callback load on the laptop (estimated ~30 %
  of budget while a knob turns; container runs had xruns and two stream stalls after xrun
  bursts at 256 frames); the Composition 3–5 ms watch item (not investigated here).
  Follow-up candidates: compile mono-`midi.in` chains once; recover a stalled stream.

## Accepted limitations

Block-rate modulation; no hysteresis on stepped destinations; pitch full scale ±60 st; state
carry O(modules²) per swap; one cable per jack input; quiet engine output; Pi 4 unmeasured;
drawer sliders lack Shift/Escape; selector routes show no reachable-options bracket; lanes pan
into view only on inspection; no `labels_on_art` contrast warning; no built-in skin;
`docs/modulation-slice/xdotool-walkthrough.sh` is stale for the rack (use `drive.py`).

## Not in scope

New synthesis, full rich LFO, plugin work, compact layout, framework change, MIDI sync,
probability/swing, tape modeling.
