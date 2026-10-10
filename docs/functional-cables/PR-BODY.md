## Functional cables: step pattern, probability, morph and glide on a cable

Draft, do not merge. Status: engineering-complete as far as this list says; not owner-accepted. Morph is kept as built on an assumption that Kosta has not confirmed (see Morph).

### What it does
A cable (jack cable or modulation route) can carry a step pattern and a pass probability, locked to the patch clock. Edit it from a jack cable's right-click menu ("Pattern & probability...") or the "Pattern" button on a route row in the routing drawer; a functional cable shows three small bars on the wire (placeholder mark, no visual design). A second pattern B with a morph A to B and a glide time can be set in the same editor.

### Semantics (rationale and rejected alternatives: `docs/decisions.md`, "Functional cables")
- Stored as cable params: `length` (0-16), `s1..s16` level, `prob` %, `r1..r16` per-step chance %; morph adds `b.length`, `b.s1..16`, `b.r1..16`, `morph` (0..1) and `glide_ms`. No new op. The old unused `steps`/`SetCablePattern` stay inert so old files cannot change meaning.
- Tick = clock pulse (16th) of the lowest-id clock; step = tick % length. Pulses come from the clock's own pulse list, so changes land on the exact sample and Stop/Run/Restart/host sync are the clock's. No clock: plain cable.
- Probability: one draw per tick from a hash of (cable id, tick); no stream state, edits never shift other draws, Restart replays the same draws.
- Morph: the pass chance and the level are the blend of the A step and the B step (each pattern keeps its own length), drawn once. Exact at both ends: morph 0 is the cable without it and morph 1 is pattern B alone, bit for bit. Glide slews level changes over `glide_ms`.
- Audio: 1 ms slew on level changes (more with glide). Pitch: a closed step holds the last pitch. CV/gate: hard edges.
- Routes are read per 64-sample block, so a route's pattern moves at the next block start.
- Schema v6 (older builds refuse v6 files; v1-v5 files load and render bit-identically). Gaining/losing the first pattern or morph compiles (crossfaded swap); every other edit, including morph, glide and pattern B, is a runtime value (`RuntimeTarget::Cable`).

### Routes into a cable's own parameters (new in this update)
A modulation route can now target a cable's Chance (`prob`), Morph or Glide (`PortRef::CableParam`), so one macro, controller or LFO moves the morph of several cables at once, each at its own depth (signed). Morph stays a value stored on each cable (Kosta's decision); a macro is CC-learnable and a host parameter, so morph is now automatable from a host.
- The route is a cable like any other: it keeps `amount` and `bypass`, can carry a pattern of its own, and amount edits are runtime values. Added, removed and bypassed through the cable's Pattern editor ("moved by" lists under Chance, Morph, Glide, stock widgets); the routing drawer lists all of them.
- Nesting rule, enforced by the compiler with an error that names the fix: a route must not target its own parameters or close a loop of such routes; chains are allowed. Reasoning in `docs/decisions.md`, "Routes into a cable's own parameters".
- Timing: all three parameters are read at each pulse, at the pulse's exact sample, and held to the next pulse; no allocation or lock on the audio path.
- Removing a cable removes the routes into it; undo restores them. A route that crosses a group boundary is refused when the group is made.
- Demo: `patches/functional-cables/macro-morph` (one macro, three cables at +100 %, +50 %, -100 %) and `docs/functional-cables/macro-morph.mp3`. Morph Suite re-rendered: bit-identical to the previous head.

### Verification (final head)
- `cargo test --workspace`: 708 passed, 0 failed, 24 ignored (18 new: `crates/engine/tests/cable_routes.rs` for timing against the source's own samples, restart replay, three depths and signs, runtime amounts, the nesting errors, undo, save and reload, a v5 file, no allocation, the demo patch; node tests in `kabl-cables`; the editor through real egui input; the plugin state). `cargo clippy --workspace --all-targets -- -D warnings`: clean. rustfmt on the files touched (workspace `cargo fmt --check` already fails on untouched files at origin/master).
- Pattern and probability: timing vs the clock sample for sample, incl. restart, stop/run, host sync with loop jump and stopped stretch; reproducibility; legacy (bit-identical render of all 22 shipped patches + 2 v1 fixtures); undo, save/reload, remove/restore; runtime edit with no rebuild; no-alloc under edits; voice lanes; UI via real egui input; production control path.
- Morph and glide, same standard: ends bit-identical to the plain cables, replay after Restart/Stop/Run, runtime edits of morph, glide and pattern B without a rebuild (and a plain cable gaining a morph needing a compile), glide timing, undo/save/reload, no allocation while they run.
- Plugin and host (`docs/functional-cables/HOST.md`): CLAP validator 35 success, 9 skipped, identical to the recorded baseline (re-run at this head). Second REAPER 7.75 run: a macro driving three cable morphs under a recorded automation envelope; saved, REAPER quit, reopened, saved: states identical, renders bit-identical, and without the envelope the render differs. Loudness envelope against the engine's own render of the same automation: correlation 0.91 (not equality). REAPER 7.75 (scripted, private Xvfb): a project with Morph Arc and Echo Throws (pattern, probability, morph) saved, REAPER quit completely, reopened and saved again: every state identical, and the render after reopen identical to the first (relative difference 0.0); the same project without the cable params renders differently. An old project comes back unchanged.
- **Bug found by the host check and fixed here:** the plugin's state validator (and the composite validator) rejected any cable with more than 32 params; a cable with pattern A, B and morph has up to 71, so such a project would not reload in a host. Both now use `kabl_cables::MAX_CABLE_PARAMS`; test `state_accepts_a_cable_with_every_functional_param`.
- Fresh reviewer subagents: `docs/functional-cables/REVIEW.md` (feature), `docs/promo-demo/REVIEW.md` (promo recording).
- Cost of routes: about +6 ns for one route in the minimal patch, ~95 ns per route over 57 routes on `patches/composition` (noisy shared machine), under 0.5 % of the block budget (`docs/benchmarks.md`). Cost: `docs/benchmarks.md` (node about 45 ns; real gating up to ~300 ns per cable on the densest patch, 1.3 % of the block budget with 57 functional cables; morph and glide add nothing measurable).

### Demos
`patches/functional-cables/{echo-throws,five-against-eight,morph-arc}` with clips in `docs/functional-cables/*.mp3`, generated by `cargo run -p kabl-ui --example functional_cable_demos`. New: a 2:48 promo, "Morph Suite" (`docs/promo-demo/`): one command (`docs/promo-demo/record.sh`) rebuilds the patch, renders the audio and records the real `kabl-ui` playing it (scripted xdotool on Xvfb, captions, the app's own audio). Kosta chose the audio (v5); he has not watched the video. To be re-recorded after the interface redesign (only `targets.json` should need changes). Adds `aim` and `slide` commands to `docs/rack-migration/drive.py`.

### Morph: an assumption for Kosta to confirm
The repository never defined morph. Three candidates were prototyped (glide, A/B blend, timed transition). Kosta worked with the blend plus glide on Morph Arc and the promo and said it works, but has not named a choice. This PR keeps the blend and glide as built and leaves timed transition as a runtime value anyone can drive. Nothing was deleted; the params are unreleased (schema v5) and can change. Kosta has since decided: morph stays a property of each cable and a shared source moves it (the section above). Still open for him: the promo could be recut with one macro instead of seven one-at-a-time moves, after the redesign.

### Unverified
- How the clips and the promo sound and look to a person beyond what Kosta has heard (checked by measurement, not by the author).
- Other hosts and operating systems; the plugin editor in a host; real-time behaviour in a host; Pi 4 timing; hands-on feel of the editor.
- Cues setting macro targets for timed morphs (out of scope, a later task); the cable editor's look (stock widgets only); a host measurement that the automation lands on the right pulse sample (engine test only); rebasing onto the FM and wavetable branches.
- Node state (slew, held pitch) is not carried across a recompile; only a Restart in the swap block could differ.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

https://claude.ai/code/session_01K1g8cKAR2ph9Ve6fxqn2aY
