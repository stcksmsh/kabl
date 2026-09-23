# Modulation slice: LFO → ADSR Attack, in production `kabl-ui`

First production modulation slice (2026-09-23). Everything here runs in the real `kabl-ui`
binary, the real op log and the real engine. The revision-2 prototype
(`crates/ui/examples/rev2_proto`) was used as a visual reference only; none of its snapshot
history or fake DSP was transplanted.

## Launch

```sh
cargo run --release -p kabl-ui -- --patch patches/reference            # 1440×900
cargo run --release -p kabl-ui -- --patch patches/reference --size 1280x800
```

MIDI: the first port that isn't ALSA's "Midi Through" (same rule as `kabl-standalone`).
The reference patch is `patches/reference/` (schema v2 op log, real history). It was built by
`cargo run -p kabl-ui --example reference_patch -- write patches/reference`.

Patch: `midi.in → osc.va (saw) → filter.svf → vca → out`, `env.adsr` on the VCA's CV.

| Knob | Routes |
|---|---|
| ADSR Attack (60 ms) | LFO #7 (0.4 Hz TRI) +30 %, MIDI velocity −25 % |
| Filter Cutoff (1.4 kHz) | LFO #7 +12 %, LFO #8 (3.1 Hz SIN) +5 %, ADSR out +30 %, velocity +15 % (four-source case) |

## Walkthrough (play / edit / save / reload)

1. Play notes. The envelope timing selector under the ADSR knobs shows **CONT** (default).
2. **Add a source:** press on an LFO's `out` jack and drag. Knobs and inputs get a blue outline;
   over a knob the hint says `Release to modulate Decay (+25 %)`. Release: the route is created,
   selected and shown in the drawer. (Click `out`, then click an input, still patches a jack.)
3. **Two sources:** click the Attack knob. It has two routes, so nothing is selected and the
   drawer says `No source selected`. Dragging the ring now changes nothing.
4. **Select and edit:** click `MIDI In #1 velocity` in the drawer (or click its cable). Drag the
   **ring** (outer band) up/down: only that route's amount changes; a pill shows
   `MIDI In #1 velocity: -5 %`. Drag the **knob body**: only the base changes.
5. **Ring reading:** faint arc = everything the active routes can reach, summed and clamped;
   strong arc = the selected route's own span; dot = where its positive peak lands; amber tick =
   clamped at that end. The drawer prints both ranges in units. These are computed from each
   source's nominal range, not measured, and the drawer says so.
6. **Invert / Bypass / Remove** in the route row. Bypass keeps the amount. Removing the selected
   route clears the selection; the ring edits nothing until you pick another source.
7. **Base entry:** type into `Base` (`25 ms`, `1.5 s`, `2k`), Enter.
8. **Cables:** toolbar `All / Focus / Hidden`. Nothing moves. Hidden shows `< LFO` /
   `< 2 mods` under modulated knobs and `> 2` at source jacks; the ring still works.
9. **Envelope timing:** click **KEY**. Now Attack/Decay/Release are captured at each note-on
   and held for that note, release included. Back to **CONT**: times follow modulation live.
10. **Undo/redo:** Ctrl+Z / Ctrl+Shift+Z (or the buttons). Each drag, connect, repatch, delete
    (with its cables) or route change is one step.
11. **Save / reload:** type a directory in `patch dir`, **Save**; restart with `--patch <dir>`
    (or **Load**). Routes, ids, amounts, bypass and timing mode come back, with undo history.

Four-source stress case: click Cutoff, pick each of the four rows in turn and drag the ring.

## Screenshots (real, from the xdotool run)

Real X pointer/keyboard events through winit on Xvfb + llvmpipe, pixels-per-point 1, captured
with `import`. Script: [`xdotool-walkthrough.sh`](xdotool-walkthrough.sh). Not Kosta's display.

1440×900: [reference](img/1440-01-reference.png) ·
[dragging LFO to Decay](img/1440-02-dragging-lfo-to-decay.png) ·
[two sources, none selected](img/1440-04-attack-two-sources-none-selected.png) ·
[ring drag on velocity](img/1440-05-ring-drag-velocity.png) ·
[bypassed](img/1440-08-bypassed.png) · [Hidden](img/1440-09-hidden.png) ·
[Focus](img/1440-10-focus.png) · [four sources](img/1440-12-four-sources-edited.png) ·
[KEY](img/1440-13-key-trigger.png) · [reloaded](img/1440-17-reloaded-attack.png)

1280×800: [reference](img/1280-01-reference.png) ·
[ring drag](img/1280-05-ring-drag-velocity.png) · [Hidden](img/1280-09-hidden.png) ·
[four sources](img/1280-12-four-sources-edited.png) · [reloaded](img/1280-17-reloaded-attack.png)

After each run the saved patch was checked: new Decay route at the default amount, velocity
route −25 % → −5 % (ring +30 px) → inverted +5 % → bypassed, LFO #7 route untouched,
envelope→Cutoff 30 % → 40 %, timing KEY after undo + redo. Identical at both sizes.

## Offline renders

`cargo run --release -p kabl-ui --example reference_patch -- render patches/reference docs/modulation-slice/renders`

WAVs are git-ignored (`*.wav`), so regenerate locally; the renders are deterministic (asserted
in the example; same hashes on every run):

| File | sha256 (prefix) | peak | rms |
|---|---|---|---|
| `reference-continuous.wav` | `3ae2bbd5…` | 0.214 | 0.0559 |
| `reference-key-trigger.wav` | `844044aa…` | 0.200 | 0.0498 |
| `exaggerated-continuous.wav` | `fc7bcb99…` | 0.182 | 0.0493 |
| `exaggerated-key-trigger.wav` | `e6731da2…` | 0.157 | 0.0329 |

5 s, 8 voices, four overlapping notes. "Exaggerated" = Attack 300 ms plus LFO #8 (3.1 Hz) on
Attack at +50 %, so the modes are easy to tell apart. The modes differ from sample 64 (the
second block) on; rms of the difference 0.011 (reference) and 0.039 (exaggerated). Nobody has
listened to them yet.

## Checks

- `cargo test --workspace`: all pass. New suites: `engine/tests/{live_edit,modulation,env_timing,legacy_sound}.rs`,
  `core/tests/legacy_format.rs`, `ui/tests/{editor_undo,interaction}.rs`, strengthened
  `core/tests/replay_proptest.rs`.
- Mutation checks (reverted after): MIDI to the active graph only → 5 `live_edit` failures;
  state carried at queue time instead of fade start → 3 failures; `0.0` inverse for an absent
  param → proptest failure.
- `cargo clippy --workspace --all-targets`: clean. `cargo fmt --check`: clean except the
  pre-existing `rev2_proto` example files, left untouched.
- Real-X run at 1440×900 and 1280×800 as above.

## Benchmark

`cargo run --release -p kabl-ui --example bench_reference`; i7-13700H, 8 voices all held, median
ns per 64-sample block over 25 × 4000 blocks, runs interleaved with the baseline binary:

| | ns/block | % of 1333 µs block |
|---|---|---|
| baseline e6db377, reference patch without routes | 14 000 – 16 700 (4 runs, ~15 200) | ~1.1 % |
| now, same routeless patch | 14 200 – 15 500 (~15 400) | ~1.2 % |
| now, reference patch with its 6 routes | ~17 800 | 1.34 % |
| now, same, key-trigger | ~19 600 | 1.47 % |
| audio-thread state carry per swap (`receive_swap`) | 2 800 median, 7 300 max | once per edit |

Routeless difference is inside run-to-run noise. Six routes (48 per-voice route evaluations and
16 modulated params with one `exp` each) cost ~2.4 µs per block. Pi 4 still unmeasured.

## Production vs prototype-only

Production (real editor, op log, engine, file format):
routes as typed `PortRef::Param` cables with persistent ids; amount / sign / bypass; taper-space
summing with one clamp; unipolar/bipolar/pitch source scaling; stepped params; feedback routes
with a 1-block delay; per-voice routing; envelope CONT/KEY in DSP; drag-to-knob; ring edits the
selected route; drawer; base entry; All/Focus/Hidden; exact undo/redo; save/reload; live-edit
safety (MIDI through swaps, audio-thread state carry, lock-free queue, reclamation).

Still prototype-only (rev2_proto), not built here: primary/advanced controls and in-place
expansion, per-source inner lanes, peak-handle/inspector depth variants, x-ray cable fading,
numeric amount entry by `+40 %` text on the plug, fine (Shift) drag, Esc-cancel of drags, toast
messages, themes (A-light/A-dark), skins beyond the existing `osc.va` demo, rich LFO, zoom.

## Known limitations

- All param modulation is **block rate**: one value per 64 samples (1.33 ms at 48 kHz), source
  sampled at the block's first sample. Modules that could take per-sample params don't get them.
- Key-trigger capture happens at the gate's rising edge with that block's modulated values.
- Stepped destinations have no hysteresis; a source hovering at a step boundary can flip the
  option every block.
- Mixer's four `level` params share one name, so a route to `level` reaches only the first.
  Pre-existing naming issue; the mixer is not in this slice.
- Pitch as a source uses a nominal ±60 semitones full scale.
- Voice→global routes average voices (existing compiler rule). No global-rate module has params
  today, so this is untested in practice.
- Audio-thread state carry scans module origins linearly: O(modules²) per swap; 7 µs worst case
  here, grows with big patches.
- Jack inputs still take one cable; connecting into an occupied jack replaces it.
- Not heard, not played on hardware, not seen on Kosta's display.
