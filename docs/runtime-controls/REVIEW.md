# D03-R2 + D04 review record

## Status: independent subagent review NOT done

The brief (D04.md, "Review and return") and AGENT-WORKFLOW.md require a fresh reviewer
subagent to inspect the combined D03-R2 + D04 code, fix findings, and recheck the final
product head. **That did not happen.** The review below is therefore missing, and this work
is **not** engineering-complete until it is done.

What happened:

- On 2026-09-26 a fresh general-purpose reviewer subagent was started against base `4f7e729`
  and head `e7dca9a`.
- The brief it was given covered: classification; identity and ordering; RT safety of the
  callback and control paths (the `Core` mutex, `pump`, what the callback touches);
  saturation and eventual convergence; lost discrete actions; editor independence;
  compatibility claims (`render_hash`, `transition_compare`); D03-R2; D02/D03 integration;
  and test quality.
- It was **terminated by the platform before writing any finding**:
  "You've hit your weekly limit · resets Sep 30, 10pm (UTC)" (rate_limit, HTTP 429). Its
  last recorded step was starting to reproduce the baseline render hash at `6404b21` in a
  scratch worktree. No review text exists.
- Per AGENT-WORKFLOW, the implementer's own checking is **not** relabelled as a subagent
  review. None of the verification in REPORT.md counts as independent review.

## What still has to happen

Once subagent capacity is available again (after 2026-09-30 22:00 UTC), or through a
supervisor-routed reviewer:

1. Start a fresh reviewer on base `4f7e729` and the submitted product head, which is the
   branch head. Product code there equals `b2e9fd6`. It must cover the list above and the
   brief's explicit points:
   - diagnosis defaults;
   - ownership;
   - source ordering;
   - queue overload;
   - callback work and destruction;
   - graph-generation races;
   - CC pickup and buttons;
   - document, undo and save;
   - probe freshness.
2. Record its findings here, then the implementer's responses and fixes, then a final
   recheck of the product head.
3. Refresh any evidence a fix affects.

## Coordinated review run (2026-09-26)

A coordinating agent (D04-COORDINATOR-LAUNCH.md) started a fresh general-purpose reviewer
subagent in an isolated git worktree with D04-REVIEW-LAUNCH.md. Coordinator override: the
reviewer had no commit/push permission; it returned its review, which is recorded below
verbatim. The coordinator made the fixes; responses follow in a separate section.

### Independent review (reviewer subagent, 2026-09-26)

**Target (checked with `git fetch origin` and `git rev-parse`)**
- Base: `4f7e72950a93765b79d3f229df00ce11d50e7c51`
- Reviewed head: `b2171180ae72f7d0e82cc172389ac5c191c464ad`. This is the branch `claude/d02-musical-controls-knjmgu` and it has not moved.
- `origin/master`: `0d97ed1e5010f4ac1bfdf03e90d2a4e25b397bfb`. Beyond the base it holds prompt docs only. The brief, AGENT-WORKFLOW and the launch prompt were read from there.
- `git diff --stat b2e9fd6 b217118`: docs and evidence only.
- `git diff --stat e7dca9a b2e9fd6 -- crates`: only `crates/standalone/tests/stream_error_{line,overflow}.rs`. The test was moved to its own file.

This is a review only. No product code was changed and nothing was committed or pushed. The reproducers were run in the isolated worktree and then deleted.

#### Coverage

Code read in full (the diff plus the surrounding code):
- engine: `runtime.rs`; `compile.rs` (slots, `set_runtime`, ramps, param/route compilation); `patch_engine.rs` (`receive_swap`, `set`, `drain`, `transport`/Toggle, `launch`, `command`, promotion of the pending graph);
- ui: `control.rs` (whole file); the `main.rs` callback, `Core`, `pump`, `App::ui` and MIDI sink; `perform.rs` (`apply_cc`, `rearm`, takeover, `fire`); `inspect.rs` (`needed_inputs`, `diagnose`, `upstream`); `browser::replace_patch` and `perform`; `library::read_patch`; `compare::restore`;
- standalone: `lib.rs::stream_error_line` and `main.rs`;
- modules: `clock.rs` (Toggle) and the `filter_ladder` process;
- basedrop 0.1.3 `Collector` drop semantics;
- tests: `crates/engine/tests/runtime_controls.rs`, `crates/ui/tests/runtime_controls.rs` and the D03-R2 tests in `signal_inspection.rs`.

Docs read: `docs/runtime-controls/{design,REPORT,README,REVIEW}.md` and `classification.md` (the ramp list), plus the Performance and Compatibility evidence.

Areas covered:
- classification and defaults/aliases (`effective` matches the compiler's lookup);
- route amount and bypass;
- identity and revision rules;
- active, incoming and pending graphs; stale-graph refusal;
- coalescing and saturation; the `MAX_GRAPHS_QUEUED` and `MAX_HELD` bounds; convergence;
- per-callback work and deallocation (Owned drops only through the collector);
- Core lock scope; failed compile (`sent` is set to the failing document by design, §6); the requested/applied counters;
- CC pickup, buttons, the reconnect guard, Toggle and learn;
- save, undo, redo and comparison restore;
- inspector generation handling;
- D03-R2 needed inputs and error-message age.

No defect was found in:
- revision ordering inside the single FIFO;
- stale graphs and reused ids of another kind;
- delete/recreate and new-document rejection of old values;
- final-value convergence under a paused consumer;
- the callback being allocation-free on the changed paths;
- save including CC edits made with no frame drawn;
- pickup, button edges and the reconnect guard;
- Toggle being resolved on the audio thread;
- the message-age wording.

#### Findings

**R-01: major, confirmed failure (an unapproved audible compatibility change).**
- **Location:** `crates/engine/src/compile.rs`, `MAX_RAMPS` (≈l.214) and `CompiledPatch::set_runtime`, the `None =>` arm at the `ponytail` comment (≈l.1811).
- **Failure scenario:** one document edit changes more than 32 ramped targets. Ramped targets are ramped params plus route amounts; mixer levels, VCA gain, SVF cutoff/resonance, ADSR times, osc pitch and similar are all in this group. The edit can be a comparison restore, the Undo that brings back "my version", or an undo/redo of a multi-knob step. The first 32 targets (in revision order) ramp over 15 ms. Every further target, including mixer levels and VCA gains, jumps in one block.
  - Before D04, the same edit compiled a graph and crossfaded everything over 15 ms.
  - This is reachable in factory material. Counts of ramped params plus param routes:

    | Patch | Ramped params + routes |
    |---|---|
    | composition | 77 + 19 |
    | performance | 77 + 7 |
    | echo | 32 + 4 |
    | crowded | 31 + 11 |
    | interlocking | 27 + 3 |
    | sequence | 15 + 0 |
    | init-keyboard | 11 + 0 |
- **Evidence (executed):** in `r_restore_past_max_ramps_steps_the_rest` (code below), Composition, capture a reference, edit 75 ramped params, then Restore.
  - The result is `Values(75)`.
  - After one 64-sample block: 32 targets are ramping and **43 of 75 are already at their final value**. The stepped ones include the remaining mixer levels and VCA gains.
- **Impact:** amplitude and filter steps can click. That is an audible behaviour change against the baseline, and the brief says such a change is Kosta's tradeoff. design.md §12 and REPORT "Limits" list it as a rule, but it is not flagged as a decision for Kosta and not in the transition comparison. This review does not accept it on his behalf.
- **Requested correction** (one of):
  - (a) Size the ramp table at compile time for every rampable target in the graph (preallocated off the audio thread), so there is no ceiling.
  - (b) When one sync would produce more than `MAX_RAMPS` ramped values, compile and crossfade instead, as before.
  - (c) Put it explicitly to Kosta as a product tradeoff, with an A/B render.

  Then add a regression test with more than 32 targets.

**R-02: minor, confirmed failure (the ordering claim is inaccurate).**
- **Location:** `crates/ui/src/control.rs::deliver` (≈l.269–296); `Delivery::flush`; the `main.rs` callback order (drain, then transport, then commands, ≈l.467–483); design.md §5, "their order relative to values does not matter".
- **What is guaranteed:**
  - Graphs and values keep revision order, because they share one FIFO.
  - Commands keep their order among themselves (FIFO), and transport keeps its own order.
  - Across the two, a value or graph is ahead of a command only if it was already **pushed** before the audio thread's `drain` ran in the same callback.
- **What is not guaranteed:**
  - (i) Anything held in `Delivery` (queue full, or `MAX_GRAPHS_QUEUED` reached) is overtaken, because commands are pushed immediately.
  - (ii) A value pushed after `drain` but before the command pops is applied one callback after the command.
  - (iii) `launch` resolves against incoming/active, not pending. A launch for a document still pending (a Load behind a fade) resolves against the old graph. This existed before D04.
- **Evidence (executed):** `r_commands_overtake_held_values` (code below).
  - Setup: fill the queue (audio paused), edit seq #6 `p1` (−20 → −13) and clock bpm (112 → 90), then Launch (Now) and Restart. One callback runs.
  - Launch and Restart were consumed while the graph still had `p1 = -20` and `bpm = 112`. Values converged later.
  - One Restore touching more than 256 seq step params is enough to hold values without any audio stall.
- **Impact:** a launched bank's first step can play its pre-edit data, or Restart can use the old tempo, for one callback. This needs saturation or a narrow race. The base had the same class of issue with graphs, so it is not a regression. The design statement is still false.
- **Requested correction:** fix design.md §5/§12 to state the guarantee above. Preferably also keep commands behind held messages:
  - either carry `Command` and `Transport` as `ToAudio` variants, which never coalesce;
  - or have `deliver` send commands only when `held` is empty and there is no `held_graph`.

**R-03: minor, confirmed failure (D03-R2 states an inference as a fact).**
- **Location:** `crates/ui/src/inspect.rs::diagnose`, the missing-input fact (≈l.720–726): "…an unplugged input reads silence, so this stage passes nothing."
- **Failure scenario:** a ladder at resonance ≥ ~0.91 (or a delay or reverb tail) keeps sounding after its `in` cable is removed, because state carries across the swap.
- **Evidence (executed):** `r_self_oscillating_ladder_after_unplug`.
  - Chain: noise → ladder (resonance 1, cutoff 800) → out. Unplug `in` and swap.
  - Peak 1–2 s later: **0.146**.
  - `diagnose` still lists "…so this stage passes nothing."
- **Impact:** a graph fact is contradicted by what the user hears or measures. The brief requires facts to be kept separate from inferences.
- **Requested correction:** keep the graph fact ("No cable into X in, its audio input"). Drop "so this stage passes nothing", or move it to `possible` with a caveat (self-oscillation or tails), and add a test.

**R-04: minor, evidence gap (lock scope compared with the latency claim).**
- **Location:** `crates/ui/src/main.rs::App::ui`. The Core lock is taken at ≈l.1044 and held until the function returns (≈l.1320).
- **What the lock actually covers:** the whole frame's logic, not just "one frame's layout" (design §11/§12). This includes:
  - `show()`, which includes browser Open/Save through `library::read`, `read_patch` (file I/O plus a validation compile) and `save_folder` (writes and renames);
  - MIDI `connect` and reconnect;
  - `KABL_STATS_FILE` and `KABL_LATENCY_FILE` writes;
  - `Delivery::sync` compiles (≤0.47 ms in the logs);
  - `collector.collect()`, which frees retired graphs.
- **What was measured:** CC-to-callback latency only in the cc-only workload (p99 8.5–8.9 ms, max 16–24 ms). There was none during swaps, Save, Open or MIDI reconnect.
- **Impact:** the bound is stated but not proven. A CC can wait for file I/O. This is not a correctness failure.
- **Requested correction:** either correct the claim to "one editor frame's logic, including any file operation or port connect in it", or measure CC latency during swaps and Save/Open. Optionally, move file operations or the connect out of the lock.

**R-05: nit, nonblocking follow-up (a new document's values reach the outgoing graph).**
- **Location:** `crates/engine/src/patch_engine.rs::set` (≈l.809). The only guard is `s.rev > g.rev`.
- **Scenario:** a Load compiles fresh graph P at rev N. A knob in the new document at rev N+k also applies to the outgoing `active` or `incoming` graph of the previous document when it has a module with the same id and kind. This lasts for the fade, plus the pending wait (up to about 30 ms).
- **Evidence:** source reading only, not executed.
- **Impact:** an inaudible-scale cross-document write while the old graph fades out. The brief's direction ("cannot target a new sound") holds; this is the reverse direction.
- **Requested correction:** skip graphs older than the newest fresh graph received (for example, record `fresh_rev` in `receive_swap`), or document the reverse direction.

**R-06: nit, evidence gap (callback allocation check).**
- The allocation check runs `assert_no_alloc` around a **copy** of the callback: `H::callback` covers drain, transport, commands and `process_block`.
- It leaves out the real `main.rs` closure's key events, the seqs/clocks/delays/lfos report pushes, the probe push, the ring and tap, and `applied_tx`. Those are unchanged from the base except the measurement-only push.
- design.md §11's "production calls" is accurate. Any statement that implies full-callback coverage should say it is a copy.

#### Reproducer (the scratch file was `crates/ui/tests/review_repro.rs`, now removed)

```rust
use std::sync::Arc;
use basedrop::{Collector, Owned};
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::compile::{compile, MAX_RAMPS};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, Launch, PatchEngine, Timing};
use kabl_engine::runtime::{ramped, Feedback, ToAudio};
use kabl_modules::builtins::Transport;
use kabl_modules::registry;
use kabl_ui::control::{self, Delivery, Outcome};
use kabl_ui::{compare, perform, routing, PatchEditor, UiState};
const SR: f32 = 48000.0; const VOICES: usize = 8;
fn load(n: &str) -> kabl_core::PatchLog { kabl_core::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches").join(n)).unwrap() }
struct H { editor: PatchEditor, ui: UiState, d: Delivery, engine: PatchEngine, rx: rtrb::Consumer<ToAudio>,
  commands: rtrb::Consumer<Command>, transport: rtrb::Consumer<(ModuleId, Transport)>, fb: Arc<Feedback>, collector: Collector }
impl H {
  fn new(n: &str) -> Self { let collector = Collector::new(); let editor = PatchEditor::from_log(load(n));
    let mut engine = PatchEngine::new(&collector.handle(), editor.state(), SR, VOICES).unwrap(); engine.active_mut().rev = 1;
    let (tx, rx) = rtrb::RingBuffer::new(control::QUEUE); let (ctx, commands) = rtrb::RingBuffer::new(64); let (ttx, transport) = rtrb::RingBuffer::new(64);
    let fb = Arc::new(Feedback::default());
    let d = Delivery::new(Some(tx), Some(ctx), Some(ttx), collector.handle(), fb.clone(), SR, VOICES, Some(editor.state()));
    let mut h = H { editor, ui: UiState::default(), d, engine, rx, commands, transport, fb, collector }; perform::rearm(&mut h.ui, 0.0); h }
  fn callback(&mut self, blocks: usize) { // main.rs order
    self.engine.drain(&mut self.rx, control::QUEUE, &self.fb);
    while let Ok((id, t)) = self.transport.pop() { self.engine.transport(id, t); }
    while let Ok(c) = self.commands.pop() { self.engine.command(&c); }
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]); for _ in 0..blocks { self.engine.process_block(&mut l, &mut r); } }
  fn settle(&mut self) { for _ in 0..8 { self.d.flush(); self.callback(16); } self.collector.collect(); }
  fn deliver(&mut self) -> Outcome { control::deliver(&mut self.editor, &mut self.ui, &mut self.d) }
}
#[test] fn r_restore_past_max_ramps_steps_the_rest() {           // R-01
  let mut h = H::new("composition"); h.settle(); h.ui.compare.capture(&h.editor, "review");
  let s = h.editor.state().clone(); let mut edited = Vec::new();
  for (&id, m) in &s.modules { let info = registry::info_for(&m.kind).unwrap();
    for p in info.params.iter().filter(|p| ramped(info.kind, p)) { if m.kind == "clock" || m.kind == "midi.in" { continue; }
      let n = p.to_norm(routing::base_value(&s, id, p)); h.editor.set_param(id, p.name, p.from_norm(if n > 0.5 { n - 0.3 } else { n + 0.3 }));
      edited.push((id, info.params.iter().position(|q| q.name == p.name).unwrap())); } }
  h.deliver(); h.settle(); compare::restore(&mut h.editor, &mut h.ui); h.deliver(); h.callback(1);
  let doc = h.editor.state().clone(); let g = h.engine.active_mut(); let mut stepped = 0;
  for &(id, i) in &edited { let p = registry::info_for(&doc.modules[&id].kind).unwrap().params[i];
    let want = routing::base_value(&doc, id, &p); if (g.param_value(id, i).unwrap() - want).abs() <= 1e-5 * want.abs().max(1.0) { stepped += 1; } }
  assert_eq!(g.ramps(), MAX_RAMPS); assert!(edited.len() > MAX_RAMPS && stepped >= edited.len() - MAX_RAMPS); // 75 edited, 43 stepped
}
#[test] fn r_commands_overtake_held_values() {                    // R-02
  let mut h = H::new("composition"); h.settle();
  for k in 0..300 { h.editor.set_param(32, "level1", 0.2 + (k % 50) as f32 * 0.01); h.deliver(); }
  assert_eq!(h.rx.slots(), control::QUEUE);
  let p1 = registry::info_for("seq").unwrap().params.iter().position(|p| p.name == "p1").unwrap();
  let before = h.engine.active_mut().param_value(6, p1).unwrap();
  h.editor.set_param(6, "p1", before + 7.0); h.editor.set_param(1, "bpm", 90.0); assert!(matches!(h.deliver(), Outcome::Values(2)));
  h.ui.launches.push(Command::Launch(Launch::new(1, Timing::Now, &[(6, 1)]))); h.ui.transport.push((1, Transport::Restart)); h.deliver();
  h.callback(0);
  assert!(h.commands.is_empty() && h.transport.is_empty());
  assert_eq!(h.engine.active_mut().param_value(6, p1), Some(before)); // launch/Restart ran on pre-edit step data and bpm
  h.settle(); assert_eq!(h.engine.active_mut().param_value(6, p1), Some(before + 7.0));
}
#[test] fn r_self_oscillating_ladder_after_unplug() {             // R-03
  let mut e = PatchEditor::new(); let z = Vec2 { x: 0.0, y: 0.0 };
  let noise = e.add_module("noise", z); let lad = e.add_module("filter.ladder", z); let out = e.add_module("out", z);
  e.set_param(lad, "resonance", 1.0); e.set_param(lad, "cutoff_hz", 800.0);
  let m = |id, p: &str| PortRef::Module { id, port: p.into() };
  let c = e.connect(m(noise, "out"), m(lad, "in")); e.connect(m(lad, "out"), m(out, "left")); e.connect(m(lad, "out"), m(out, "right"));
  let col = Collector::new(); let mut eng = PatchEngine::new(&col.handle(), e.state(), SR, 1).unwrap();
  let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]); for _ in 0..750 { eng.process_block(&mut l, &mut r); }
  e.disconnect(c); eng.receive_swap(Owned::new(&col.handle(), compile(e.state(), SR, 1).unwrap()));
  let mut peak = 0f32; for k in 0..1500 { eng.process_block(&mut l, &mut r); if k > 750 { peak = peak.max(l.iter().fold(0f32, |a, x| a.max(x.abs()))); } }
  let clocks = std::collections::HashMap::new();
  let d = kabl_ui::inspect::diagnose(e.state(), Some(lad), &kabl_ui::inspect::Context { clock_running: &clocks, output_level: Some(peak), tap: None, midi_input: None, sample_rate: SR, output: None });
  assert!(peak > 0.1); assert!(d.facts.iter().any(|f| f.contains("passes nothing"))); // peak 0.146
}
```

#### Commands run by this reviewer

All from the worktree, with `CARGO_TARGET_DIR` in the scratchpad.

| Command | Result |
|---|---|
| `cargo test -p kabl-engine --test runtime_controls` | 8 passed, 1 ignored |
| `cargo test -p kabl-ui --test runtime_controls --test signal_inspection --test perform` | 8 + 24 + 12 passed |
| `cargo test -p kabl-ui --test review_repro -- --nocapture` (scratch file, later removed) | 4 passed; confirms R-01, R-02 and R-03 with the numbers above |
| `cargo test --workspace` (CARGO_INCREMENTAL=0, debug=0) | **540 passed, 0 failed, 16 ignored**, the same as the implementer's figure |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

The first build ran out of disk space. The scratch target was rebuilt without debuginfo; the product code was unchanged.

#### Limitations

- No GUI, audio device or MIDI was run. The perf, latency, walkthrough and package evidence were read, not re-run.
- R-04 is a gap in the evidence, not a measured latency failure.
- R-05 is from source reading only.
- No listening. Whether R-01's steps actually click is for Kosta.
- I did not review the D03-R1 overflow exception beyond confirming it is untouched.

#### Recommendation

**Changes required before engineering completion.**
- R-01: fix it, or get Kosta's explicit decision on the tradeoff.
- R-02: correct design.md at minimum; keeping commands behind held messages is recommended.
- R-03: correct the fact wording and add a test.
- R-04: correct the claim or measure it.
- R-05 and R-06 are nonblocking.

After the fixes, a final recheck of the product head is needed.

## Implementer (coordinator) responses

Fixes on top of the reviewed head `b217118`. Product fix commits: `dbce9bc` (R-03), `d717685`
(R-01), `05179e1` (R-02, R-05 and the regression tests). Docs: `69c8621` (this record), the
design/checklist commit after `05179e1`, and the evidence commit that adds this section.
Fixed product head: `05179e18c0d2c96760bf46fc5e095556f80aa413`.

| ID | Disposition | What changed | Check |
|---|---|---|---|
| R-01 major | **fixed** (correction (a)) | `compile.rs`: `MAX_RAMPS` removed. `CompiledPatch::ramps` is a `Vec<Ramp>` whose capacity, allocated in `compile` (off the audio thread), is the number of distinct rampable targets (ramped params by (id, index) plus routes by cable). One ramp per target, so `push` never exceeds capacity; an unreachable `else` sets rather than allocate. Removal uses `swap_remove`. No product tradeoff left for Kosta. | `a_restore_of_many_targets_ramps_every_one` (ui, Composition: every ramped non-clock param edited, then Restore; all ramp after one block, none stepped, then land exactly; runs under `assert_no_alloc`) |
| R-02 minor | **fixed** (the recommended correction, not only the doc) | `runtime.rs`: `ToAudio::Command`, `ToAudio::Transport`; `Feedback::actions_taken`. `patch_engine.rs::drain` applies them in queue order. `control.rs`: the separate command/transport queues are gone; `Delivery::command`/`transport` hold actions (≤ `MAX_HELD_ACTIONS` = 64, else refused and reported) tagged with the current revision; `flush` sends the held graph, then values and actions merged by revision. `pending()` counts actions. `main.rs`: the callback's separate transport/command loops are removed. design.md §5/§6/§12 state the exact guarantee, including (iii) `launch` against a pending Load (pre-existing, documented, not changed). | `commands_never_overtake_earlier_edits` (the reviewer's scenario: full queue, `p1` and bpm edits, Launch Now + Restart; asserts that when any action is taken both edits were already applied) |
| R-03 minor | **fixed** | `inspect.rs::diagnose`: fact is now "No cable into X in: that is its audio input."; the silence inference moves to *possible* with the self-oscillation/tail caveat. Checklists updated to the new wording. | `why_no_sound_names_an_unplugged_intermediate_audio_input` extended (no fact claims silence; caveat present) |
| R-04 minor | **claim corrected** (not measured) | design.md §11/§12 and REPORT now say the lock covers one editor frame's whole logic, including Open/Save file I/O, port connect, stats writes, compiles and collection; measured latency covers the cc workload only. No code moved out of the lock. | docs only; remains an evidence gap, listed in REPORT limits |
| R-05 nit | **fixed** | `patch_engine.rs`: `fresh_rev` recorded in `receive_swap` for a fresh (Load) graph; `set` skips graphs with `rev < fresh_rev`. | `a_new_documents_values_skip_the_outgoing_graph` (fails without the guard: old graph moved 0.25 → 0.297) |
| R-06 nit | **wording corrected** | design.md §11 says the allocation check wraps a copy of the callback's control part and lists what it leaves out. | docs only |

Verification at `05179e1` (crates identical at the docs head):

- `cargo test --workspace`: **543 passed, 0 failed, 16 ignored**, exit 0 (540 + the three new
  tests). `evidence/review-fixes/test-workspace.txt`.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean. `evidence/review-fixes/clippy.txt`.
- `render_hash` (the same 17 sounds, 20 s): byte-identical output to `evidence/render-hash-final.txt`,
  so still bit-identical to baseline `6404b21`. `transition_compare`: byte-identical to
  `evidence/transition-compare.txt`. Copies in `evidence/review-fixes/`.
- D03-R2 screenshots (`img/{1440x900,1280x800}-{light,dark}-why-audio-input.png`,
  `*-dark-why-undone.png`) re-shot on the release build of `05179e1` (same Xvfb/null-sink/fifo
  setup): they show the new wording. The previous images remain in git history at `b217118`.
- Not re-run, with reasons: perf runs and package (the fixes change no path those workloads
  exercise at scale: single-target ramps, no commands in knob/cc/swaps loops; a restore with
  >32 targets was not in them); the walkthrough video and its `wt-07` frame, recorded on
  `e7dca9a`, still show the pre-R-03 fact wording and the pre-R-02 command queues.

## Recheck

The same reviewer subagent (resumed with its context, D04-RECHECK-LAUNCH.md, same no-commit
override) rechecked `05179e1`/`dcd3a46`. Recorded verbatim:

### Final recheck (reviewer subagent, 2026-09-26)

**Target (checked with `git fetch origin` and `git rev-parse`)**
- Base: `4f7e72950a93765b79d3f229df00ce11d50e7c51`
- Previously reviewed head: `b2171180ae72f7d0e82cc172389ac5c191c464ad`
- Current branch head: `dcd3a4623fe262d461f8c109437f2d0936a19669` (`claude/d02-musical-controls-knjmgu`)
- Fixed product head: `05179e18c0d2c96760bf46fc5e095556f80aa413`. `git diff --stat 05179e1 dcd3a46 -- crates` is empty, so `dcd3a46` is a docs-only head on top of it.
- `origin/master`: `0d97ed1e5010f4ac1bfdf03e90d2a4e25b397bfb`, still planning docs only. No integration with master is needed.

What I read:
- the full diff `b217118..05179e1` for `crates/engine/{compile,patch_engine,runtime}.rs`, `crates/ui/src/{control,inspect,main}.rs` and the tests;
- the design.md diff;
- REVIEW.md "Implementer (coordinator) responses".

Each disposition was checked in code, not from the prose.

#### Per-finding disposition

| ID | Disposition | Verified |
|---|---|---|
| R-01 (major) | **Resolved** | See below. |
| R-02 (minor) | **Resolved for the reported scenario; one residual exception (RC-01)** | See below. |
| R-03 (minor) | **Resolved** | `diagnose` keeps the fact "No cable into X in: that is its audio input." The silence inference, with the self-oscillation/tail caveat, moved to `possible`. The test was extended. |
| R-04 (minor) | **Claim corrected; the gap remains and is disclosed** | design §11/§12 and REPORT now describe the full frame-lock scope. It was not measured. |
| R-05 (nit) | **Resolved** | See below. |
| R-06 (nit) | **Resolved** | design §11 wording. |

**R-01.**
- `MAX_RAMPS` is gone. `ramps: Vec<Ramp>` is given, in `compile`, a capacity equal to the distinct ramped `(id, index)` pairs plus the distinct route cables.
- The capacity matches the one-ramp-per-target invariant:
  - `RampOn::Param(lo, hi)` is unique per `(id, index)` within a graph.
  - Route ramps are taken for every compiled route regardless of `ramped`, and every route is counted.
  - `set_runtime` replaces an existing ramp for the same `on` before it pushes.
- A push happens only under `len < capacity`, so it cannot reallocate. The fallback arm is dead code that sets rather than allocates. `swap_remove` does not free.
- The Vec is freed with the graph through basedrop, on the collector's thread. A graph with no rampable targets has capacity 0 and never pushes.
- `a_restore_of_many_targets_ramps_every_one` runs under `assert_no_alloc` and passes.

**R-02.**
- `Command` and `Transport` are now `ToAudio` variants in the one FIFO, and `drain` applies them in queue order.
- Held actions carry the revision current when they were requested, and `flush` merges them with held values by revision.
- The callback's separate command/transport loops are removed.
- `pending()` counts actions (`actions_sent - actions_taken`) and cannot underflow. With `tx = None`, actions are refused, and they are reported only when audio exists (as before).
- A held graph always goes before held actions. design §5 discloses this ("the command then acts on the newer graph").
- The reviewer scenario (`commands_never_overtake_earlier_edits`) passes.
- Commands are now applied inside `drain`, before key events in the same callback. Previously they came after. This is harmless: these are independent sources with no defined relative order.

**R-05.**
- `fresh_rev` is set in `receive_swap` for fresh graphs, and `set` skips graphs with `rev < fresh_rev`.
- Pending case: if a fresh Load L is pending and a newer edit graph E replaces it, `fresh_rev` stays at L's revision and `E.rev ≥ fresh_rev`. The ordering of the `fresh |=` assignment does not matter.
- rev-0 unversioned graphs and startup (`fresh_rev = 0`) are unaffected.
- `a_new_documents_values_skip_the_outgoing_graph` passes.

#### New findings

**RC-01: minor, confirmed failure (the ordering guarantee in design §5 is broken by coalescing, and the stated exception is backwards).**
- **Location:** `crates/ui/src/control.rs`, `Delivery::sync`, where `held.insert(*target, s)` replaces the older value with the newer revision; and `Delivery::flush`, the merge by `s.rev`. design.md §5, "Guaranteed order".
- **Failure scenario:**
  1. With the queue saturated, edit X (seq `p1`) is made at revision r1.
  2. A Launch is requested, tagged r1.
  3. A newer edit Y of the same target is made at revision r2 > r1. It replaces X in `held`, so the value now sorts after the Launch.
  4. The Launch is sent first. If the queue fills between the two, the Launch runs and a block renders with neither X nor Y applied.
- **What design.md says instead:** the only exception it gives is "a value requested after a command may be sent before it". The code does the opposite: an edit requested before the command is sent after it.
- **Evidence (executed, scratch `crates/ui/tests/review_recheck.rs`, since removed):** `r_coalescing_lets_a_command_overtake_an_earlier_edit`.
  - Composition, queue full, 255 held values before the Launch, then X (p1 −20 → −13), then Launch Now, then Y (−15).
  - After resume, drain, flush and drain, the Launch was taken (`actions_taken = 1`) and a block rendered with `p1 = -20`.
  - It converged to −15 afterwards.
- **Impact:** only under saturation, plus the exact queue boundary. At most one callback of pre-edit step data, the same class as R-02 but much narrower. Not audible in normal use.
- **Requested correction** (either one):
  - Keep the coalesced value at its oldest ordering position when a held action lies between the two edits. For example, sort on a separate order key equal to `min(old key, new rev)`, and keep `rev` for the graph rule.
  - Or correct design §5 to state the real exception: an edit requested before a waiting command can be delivered after it when a newer value of the same target coalesced with it.

**RC-02: minor, confirmed failure, pre-existing (the base's `SwapSender` had the same behavior), not a regression.**
- **Location:** `crates/ui/src/control.rs::Delivery::compile`, `self.held_graph = Some(...)`.
- **Failure scenario:** when a held graph is replaced, its `fresh` and `stopped` flags are lost. If a Load is waiting in `Delivery` (because `MAX_GRAPHS_QUEUED` graphs are untaken) and the next structural edit replaces it, the new graph goes out with `fresh = false` and without the stopped clocks. As a result:
  - state is carried across documents by module id;
  - pending launches are not cleared;
  - a piece opened as "stopped: press Start" plays;
  - the R-05 fence is bypassed for that Load.
- **Evidence (executed):**
  - `r_held_load_replaced_by_an_edit_loses_fresh_and_stopped`: two structural edits with audio not draining, then a Load with `load_stopped = true`, then one more structural edit. After settling, clock #1 is running (`Some(true)`).
  - Control test without the extra edit: `Some(false)`.
- **Impact:** needs the audio thread to take no graphs across at least three compiles, for example during the known cloud stream stall. `MAX_GRAPHS_QUEUED = 2` makes this slightly easier to reach than the base's `SWAP_QUEUE = 4`.
- **Requested correction:** when replacing a held graph, carry its flags over: `g.fresh |= old.fresh`, and apply Stop to the clocks if the replaced graph was a stopped Load (track `stopped` in `Delivery`). Add a test. This can be a follow-up if Kosta prefers, since it is pre-existing.

**RC-03: nit, evidence gap.**
- The perf runs and the walkthrough video were not re-run on `05179e1`, although the callback path changed: commands now go through `drain`.
- REVIEW.md discloses this. The walkthrough frame `wt-07` still shows the pre-R-03 wording.
- No correction is required beyond keeping the disclosure.

#### Commands run by this reviewer

All run at `dcd3a46`; crates are identical to `05179e1`.

| Command | Result |
|---|---|
| `cargo test --workspace` (CARGO_INCREMENTAL=0, debug=0) | **543 passed, 0 failed, 16 ignored**, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, clean |
| `cargo test -p kabl-ui --test review_recheck -- --nocapture` (scratch, removed) | 3 passed, confirming RC-01, RC-02 and the RC-02 control |

The scratch copy is at `/tmp/claude-0/-home-user-kabl/e0bc9845-1c97-47d0-9470-0491a6d7c135/scratchpad/review_recheck.rs`. The worktree is clean.

Taken from the implementer's artifacts, not re-run: `render_hash` and `transition_compare` byte-identity, the refreshed D03-R2 screenshots, and the package run.

#### Limitations
- No GUI, audio device, MIDI hardware or listening.
- Perf and latency were not re-measured after the fixes.
- RC-01 and RC-02 need queue saturation or an audio stall to reach.

#### Assessment
- **No blocker or major finding remains.** No original acceptance criterion is failing in code.
- R-01 is fixed rather than accepted as a tradeoff, so no ramp tradeoff is left for Kosta.
- What remains for Kosta is owner judgment already recorded as pending:
  - the by-design change from a 15 ms crossfade to a 15 ms ramp or module smoothing on knob transitions (listening);
  - the unmeasured CC latency while the editor frame holds the lock (R-04, disclosed).
- **Recommendation:** fix RC-01 before calling this engineering-complete. A doc correction or the order-key change is enough. RC-02 is pre-existing and can be fixed now or filed as a follow-up. With RC-01 handled, I have no objection to engineering completion. A code change for RC-01 would need a short recheck of `control.rs::flush`/`sync` only.

## Coordinator responses to the recheck

Fix commit: see the commit after `a3b480b` touching `crates/ui/src/control.rs`.

| ID | Disposition | What changed | Check |
|---|---|---|---|
| RC-01 minor | **fixed** (order key) | `Delivery::held` stores `(order, ParamSet)`. A new target's order is its revision; a coalesced value keeps the order of the oldest value it replaced, while `rev` (the graph rule) is the newest. `flush` sorts and merges with actions by order. So an edit requested before a command is never sent after it; a newer value of that target may ride ahead of the command in its place, which is the exception design.md §5 already states. | `a_coalesced_edit_keeps_its_place_before_a_command` (the reviewer's scenario at the exact queue boundary; fails without the fix with `p1 = -20`) |
| RC-02 minor (pre-existing) | **fixed** | `Delivery::compile`: when the held graph it replaces is fresh (a Load), the new graph is fresh too and inherits the Load's stopped clocks (`held_stopped`). | `a_waiting_load_replaced_by_an_edit_stays_a_stopped_load` (with and without the extra edit; fails without the fix: clock running) |
| RC-03 nit | **disclosure kept** | Perf, package and walkthrough are not re-run; see REPORT "Limits". | — |
