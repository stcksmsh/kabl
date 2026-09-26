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

Pending.

## Recheck

Not started.
