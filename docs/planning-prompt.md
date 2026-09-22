# Planning session prompt: kabl

You are joining the owner (Kosta) for a planning session on **kabl**, a modular synthesizer in
Rust. Repo: `/secondary/Programming/Github/kabl`, branch `master`. You have not seen it before.
This session is for deciding **what to build next, how, and what to change or cut**. Do not
implement anything unless Kosta asks.

The result should be a prioritized plan that Kosta agrees with, written to `docs/PLAN.md`. Each
item needs scope, approach, acceptance criteria, and dependencies. Also list the decisions Kosta
still has to make.

## How to work

1. **Orient first (read, don't write).** Read `README.md`, then `docs/STATUS.md`. Skim the
   headings in `docs/decisions.md` and read the entries you need. Run `git log --oneline` (about
   40 commits). Read code only where a planning question depends on it.
2. **Check claims against the code.** `STATUS.md` was written over several autonomous sessions and
   parts of it are stale (see "Known stale spots" below). Where the docs and the code disagree,
   the code and `git log` are correct.
3. **Plan together.** Propose options and give a recommendation for each. Ask Kosta about taste,
   product, and priority questions. Don't invent answers to those. Find problems yourself: missing
   pieces, weak designs, tech debt worth paying down now, and things worth deleting.
4. **Output.** `docs/PLAN.md` as described above. Put decisions Kosta makes during the session in
   `docs/decisions.md` (append-only, same style as the existing entries). Update `docs/STATUS.md`
   only where it is wrong.

## Constraints and preferences

- **Git:** commit straight to `master`. No branches, no PRs. Each commit should be a small,
  meaningful, working state.
- **Minimal solutions:** use what already exists in the codebase, then std, then an existing
  dependency. Add a new crate or abstraction only when it is clearly worth it. Skip speculative
  work.
- **Comments:** explain why, keep them short, and never narrate. Earlier sessions over-narrated
  code comments and had to be trimmed. Design rationale belongs in `decisions.md`, not in code.
- **Honesty:** report measured numbers and what was actually verified. Several spikes missed
  their targets and were reported as misses. Keep doing that.
- **Chat style:** Kosta prefers short, token-lean replies.
- **Hardware on this machine:** real audio output, a real X display (`:1`), and a real MIDI
  controller (Arturia KeyLab Essential 61 mk3, connected when in use). Things previously marked
  "unverified, no hardware" can now be tested for real.
- **Not in the repo:** the original project brief ("brief section N" in the docs) is held by
  Kosta. Ask him when a section number matters.

## The product

Tagline: *a legible modular synthesizer. Cables are instruments, patches explain themselves,
modules can be built like guitar pedals.*

Planned milestones, as far as the docs show:

- **v1 Engine:** a playable, patchable, savable polysynth.
- **v2:** cables as nodes (depth, step patterns, probability, morph).
- **v3:** clocks and sync.
- **v4:** pedals (composite modules, code modules via Faust or WASM) and the CLAP plugin.
- **`learn`:** lessons and unlock flags. Timing unclear.

**v1 acceptance test (from STATUS.md):** play a 4-voice MIDI chord, repatch live with no click,
undo, save, reload, replay the construction log. Also pass the "potato gate" performance target
on a Raspberry Pi 4. The infrastructure for all of this exists, but no one has run it end to end
on real hardware.

## Workspace

| Crate | State |
|---|---|
| `core` | Done. Event-sourced patch: op log, undo/redo, coalescing, checkpoints, versioned file format, property tests. |
| `engine` | Flat-schedule compiler (`compile.rs`): topological sort, voice/global instancing, buffer-pool reuse, cycles compiled with an implicit one-block delay, state carried across `recompile`. `process_block` proven allocation-free. `PatchEngine` does hot-swap with an equal-power crossfade and queues overlapping swaps. `VoiceAllocator` exists. It also still contains spike code: `swap.rs` (S1), `potato.rs` (S2), `simd_voices.rs` (S3), `dyn_dispatch_spike.rs`, `patch_demo.rs`. |
| `modules` | 9 v1 built-ins: `osc.va`, `filter.svf`, `env.adsr`, `lfo`, `vca`, `ringmod`, `mixer`, `out`, `midi.in`. Also a registry, `ModuleInfo` (now with `width_units`), and the `ModuleSkin` mechanism for custom panel art and control placement. |
| `standalone` | Binary `kabl`: cpal + midir, default patch or `--patch <dir>`, `--save-default`, `--midi <substring>`. Confirmed playing live from the real controller. |
| `ui` | Binary `kabl-ui`: egui patchbay with the same audio path. Free-form Patchbay view plus a grid-snapped Eurorack view (v1, rough). Panel aesthetic, curved cables, on-panel knobs, undo/redo, Save/Load through a text path field. Never run with real audio by a person. |
| `cables`, `pedals`, `learn`, `clap` | Empty stubs. |

## Recently done (2026-09-22, not yet reflected everywhere in STATUS.md)

- `2fd0aef`: `kabl-ui` now shares standalone's MIDI port selection. It used to connect to ALSA's
  "Midi Through" loopback instead of the real controller.
- `2cf3d29`: `kabl-ui` runs `compile()` outside the engine mutex and locks only for
  `PatchEngine::finish_swap` (state transfer and install). The audio thread still uses
  `try_lock` and outputs silence when the lock is busy. A lock-free engine publish is still the
  textbook fix, but that has not been done.
- Eurorack view v1 brought back canvas pan and scroll.

## Known stale spots in STATUS.md

- The older "Handover: next session starts here" list still says there is no canvas pan and that
  nothing has run on real hardware. Both are now wrong in part.
- Several places describe the Mutex window as lasting the whole `build_swap`. That is now fixed.
- One place says both binaries connect to the first MIDI port. That is fixed in both.
- `README.md` says "nothing playable exists yet". That is wrong.

## Known gaps and debt (candidates, not commitments)

**Verification**
- The whole v1 acceptance test has never been run by a person on real hardware. `kabl-ui` has
  never been heard.
- The S2 potato gate needs a real Raspberry Pi 4. No number exists.
- S3 SIMD voice batching measured about 1.5–1.7x against a 2.5x target. It is not integrated.
  Open decision: use it in v1 anyway, or drop it?

**Engine**
- Quality tiers (Live and Render) are not built.
- The control-rate tier (S2) and SIMD batching (S3) exist only as spikes and are not in the
  compiler.
- `voice_count` is a fixed compile parameter.
- When a second edit arrives during a crossfade, the queued swap's state can be up to one extra
  crossfade stale.
- The spike code in `engine` should eventually be absorbed or deleted.
- `Box<dyn Module>` dispatch costs about 17–19% over static dispatch. This was accepted.

**Modules**
- `osc.va` triangle is naive (no BLAMP correction).
- `lfo` has no sync (needs v3 clocks).
- `Switch` and `Readout` skin control kinds are declared but never rendered.

**UI**
- Kosta said the look is "too simple and soulless" and wants it "more like a synth". The first
  pass (panel aesthetic, skins, Eurorack view) is done, but his direction is still vague.
- Open question: should there be one UI that evolves, or a simple one now and a polished one
  later? The earlier recommendation was one UI, not confirmed by Kosta.
- The Eurorack view is rough:
  - `width_units` values are guesses.
  - The skinned `osc.va` doesn't respect `width_units` for its width.
  - The grid color is untuned.
  - There is no zoom.
- Only `osc.va` has a skin, and its art is a labeled placeholder. Real panel art needs a designer
  or an image tool.
- Cables connect by click-to-click. Drag-to-connect does not exist.
- Save/Load uses a text path field. A native picker (`rfd`) has been proposed but not done.
- There is no scope, meter, or "explain" overlay, even though "patches explain themselves" is
  part of the pitch.

**Other**
- There is no JACK backend.
- `RemoveModule` undo loses param history.
- `SetParam` falls back to 0.0 for a param that was never set.
- Open questions from brief section 16, not yet forced by any work:
  - Reassigning a module between voice rate and global rate.
  - Feedback-loop semantics.
  - Whether standalone or CLAP comes first.
  - Sharing the op-log model with "Hysteresis", which seems to be another of Kosta's projects.
    Ask him.
  - When to do MIDI learn and MPE.

## Questions to get Kosta's answers on early

1. What should the next milestone be? Options: finish and prove v1 on real hardware, push the
   UI toward "feels like a synth", or start v2 cables.
2. What does "more like a synth" mean concretely? Also settle one UI versus two, and whether the
   Eurorack view becomes the main view or stays an alternative.
3. S3 SIMD and the S2 potato gate: are they still v1 requirements? Is a Pi 4 available?
4. Which spike code gets deleted, and which gets absorbed into the compiler?
5. Should the audio-thread lock become a lock-free publish now, or only after glitches are heard
   in practice?

Start by reading. Then give Kosta a short summary of where the project really stands, with every
stale-doc correction you found, and ask him the first questions.
