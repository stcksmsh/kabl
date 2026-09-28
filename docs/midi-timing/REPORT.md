# D06 engineering report

```text
Batch ID / outcome: D06 — expressive, correctly timed MIDI (bend, wheel, channel routing,
                    note identity, partition-independent sample-accurate timing)
Starting commit:    bd64165840bda6bf543191a01302a4af5165dd34 (origin/master, verified)
Submitted head:     see "Commits" below (updated at submission)
Branch / PR:        codex/d06-expressive-midi (worktree ../kabl-d06), draft PR #9 https://github.com/stcksmsh/kabl/pull/9
Engineering status: submitted; independent review in REVIEW.md
Owner-review status: pending
```

## Environment (measured this session, 2026-09-28)

Kosta's laptop via remote control: Lenovo ThinkBook 16, Intel i7-13700H (20 threads), 31 GiB,
Ubuntu 24.04.4, kernel 7.0.0-31-generic. Rust 1.93.0 (default toolchain; tests and builds);
strict Clippy with Rust 1.98.1 (installed per user with rustup, default unchanged, because
Clippy 1.93 flags pre-existing `browser.rs` code at the baseline too). Audio: PipeWire 1.0.5
(pulse server) on `alsa_output.pci-0000_00_1f.3.analog-stereo`, reached through cpal's ALSA
backend ("Default Audio Device"); negotiated 48000 Hz, 2 channels, fixed 256-frame callbacks
(stats: every callback 256 frames). RT priority granted (rtkit). Display: the user's X11 `:1`
(1920×1200) was left alone; all app runs used Xvfb (`:97` 1600×1000, `:98` 1440×900) with
llvmpipe rendering. MIDI: ALSA sequencer present; no physical controller connected (only
"Midi Through"); every live run used the virtual port `kabl-player`
(`examples/midi_player`) fed by `play_events.py` — **scripted virtual MIDI**. REAPER 7 is
installed but was not used (D07 scope; no projects or settings touched).

## Acceptance matrix

| Criterion | Status | Evidence |
|---|---|---|
| Identical timestamped performance across 1/63/64/65/127/256/512 and mixed partitions | Pass, bit-identical (tolerance 0) | `crates/engine/tests/timeline.rs` (a synthetic feedback/noise/wheel/split patch at 44.1/48/96 kHz + 7 factory patches, 3 irregular mixes); offline demo renders at 1/64/256/irregular have equal hashes (below) |
| Output equals the engine's own grid shifted by exactly the declared latency | Pass | same tests compare each sample against a direct engine run +64 frames |
| Events at frame 0, in blocks, at edges (63/64/65/127), final frames, simultaneous | Pass | `performance()` in timeline.rs; `a_note_sounds_from_its_own_sample` |
| Sample-accurate (not rounded to 64) | Pass | gate edge at `t + 64` for t = 1000/1023/1024/1025 across buffer sizes; mutation to offset 0 fails 5 tests |
| Feedback, modulation grid, ramps, clocks/sequences, noise preserved | Pass | synthetic patch with a feedback loop, seeded noise, LFO and wheel/velocity routes; performance/composition/echo |
| Graph replacement, off-thread compile, Load, panic, restart, sample rates | Pass | timeline.rs (routing swap at 64 000, Load at 76 800, panic, reset/new engine, 44.1/96 kHz) |
| Queue saturation, late/out-of-range events | Pass | `late_out_of_range_and_overflowing_events_never_strand_a_note` |
| Pitch bend: range/units, center/endpoints, held notes, mono/glide, reset | Pass | `tests/expression.rs`; UI screenshots; owner feel pending |
| Wheel as modulation source, distinct from CC mapping; CC1 mappings intact | Pass (software) | expression.rs wheel tests; `midi_messages_reach_the_wheel_and_the_mappings` (CC 1 goes to both) |
| Velocity 0, overlapping/repeated notes, sustain, stealing, notes off, panic, lost source | Pass | expression.rs, keyboard.rs, preview.rs |
| Two keyboard destinations addressed independently; layering by default | Pass | expression.rs; walkthrough (bass ALL ↔ ch 2) |
| Releases reach original recipients after routing change; deletion/replacement leak nothing | Pass | `a_note_held_across_a_routing_change…`, `deleting_or_replacing…` |
| MIDI independent of UI repaint | Pass (by construction) | MIDI thread → bounded queue → audio callback; no UI or control-thread step on the path (`on_message`, callback in `main.rs`) |
| D04 ordering / D05 fences, async compile, recovery | Pass (tests) | workspace tests; ALSA-null opt-in tests 4/4 |
| No allocation on changed callback paths | Pass | timeline/expression `assert_no_alloc`; ALSA-null production-callback counter (with note, bend, wheel, pedal) |
| Old factory patches unchanged | Pass | 17 patches × 20 s, `render_hash` bit-identical to baseline (`evidence/factory-render-hashes.txt`) |
| Persisted new settings | Pass | `bend_channel_and_wheel_route_persist_and_undo`; demo patches committed |
| Laptop 48 kHz/256 measurements, baseline vs final | Done | table below |
| Physical controller, listening, feel | **Unverified** | owner checklist |

## Verification

At product tree of commit `ebfac5f` (Rust 1.93 unless noted):

- `cargo test --workspace`: **586 passed, 0 failed, 22 ignored** (baseline `bd64165` on the same
  machine: 561/0/20).
- `cargo +1.98.1 clippy --workspace --all-targets -- -D warnings`: clean.
- `ALSA_CONFIG_PATH=<null PCM> cargo test -p kabl-ui --bin kabl-ui recovery_tests:: -- --ignored`:
  **4 passed** (repeated reopen/fault, stall mutes, warmed production callback 0 alloc/0 dealloc
  with note + bend + wheel + pedal through the new timeline, CC during 500 ms Save/Open:
  Save 6.3 ms, Open 4.2 ms).
- Prototype (separate workspace): `cd prototypes/reaper-clap && cargo build --lib` succeeds
  unchanged (it still uses the pre-D06 `PatchEngine::key` and fixed blocks; see design.md §9).

## Laptop measurements (48 kHz, 256 frames, RT on)

Method: `docs/midi-timing/scripts/measure-all.sh`; each run ≈45 s of 16th notes at 132 bpm
(`pulse.events`, 704 messages) from the virtual controller. Light = `fixtures/gate-probe`
(gate straight to the output; onsets measured in the app's own recording by `onsets.py`);
dense = `patches/sound-palette` with its sequences started. Active = a knob kept turning,
Save As, Open (a Load) and Save during the run. Baseline = release build of `bd64165`, same
machine, same scripts, same player. Execution = callback run time (µs, histogram bins of
50 µs; budget 5333 µs); arrival = worst interval between callback starts; MIDI delivery =
D06 counters (baseline has none). Raw: `evidence/measure/`.

| Run | Callbacks | Exec p50 / p99 / worst µs | >½ budget | Late / xruns | Arrival worst µs | Onset jitter vs script (std / p-p ms) | IOI error std ms |
|---|---|---|---|---|---|---|---|
| base light idle ×2 | 10066, 10078 | ≤50 / ≤100 / 153, 199 | 0, 0 | 0 / 0 | 5691, 5591 | 1.544 / 5.64 · 1.565 / 6.36 | 2.46, 2.46 |
| **D06 light idle ×2** | 10066, 10077 | ≤50 / ≤100–150 / 168, 155 | 0, 0 | 0 / 0 | 5638, 5685 | **0.122 / 1.21 · 0.202 / 2.58** | **0.14, 0.26** |
| base light active ×2 | 10058, 10046 | ≤50 / ≤100 / 121, 106 | 0, 0 | 0 / 0 | 5611, 5659 | 1.559 / 6.85 · 1.552 / 6.24 | 2.46, 2.46 |
| **D06 light active ×2** | 10048, 10047 | ≤50 / ≤100–150 / 156, 131 | 0, 0 | 0 / 0 | 5659, 6156 | **0.168 / 1.96 · 0.644 / 3.08** | **0.20, 0.28** |
| base dense active ×2 | 10036, 10036 | ≤1550–1650 / ≤2300–2400 / 4192, 4056 | 34, 32 | 0 / 0 | 5638, 5773 | – | – |
| **D06 dense active** (complete run) | 10061 | ≤1350 / ≤2250 / 3415 | 15 | 0 / 0 | 5627 | – | – |
| base dense idle (partial, see below) | 7214, 2472 | ≤1350–1450 / ≤2150–2300 / 3872, 3199 | 7, 5 | 0 / 0 | 5703, 5634 | – | – |
| **D06 dense idle** | 10058 (+1526 partial) | ≤1650 / ≤2550 / 3932 | 47 | 0 / 0 | 5738 | – | – |

D06 MIDI delivery (complete runs): 705 events each (704 + one source-lost when the player
exits), 0–3 "late" (arrived in a stretched callback period, placed at the edge), arrival to
scheduled frame 5.03–5.68 ms (one period ± callback jitter), adapter 0 dropped, 0 moved.
Earlier exploratory runs made while a cargo build ran showed up to 6 late events with ~80 ms
waits: the MIDI input thread (normal priority) was preempted between stamping and queueing.

Reading: live onset jitter falls from ≈1.55 ms std / ≈6 ms peak-to-peak (callback-start
quantization, as the offline pre-D06 emulation predicts: 1.54 ms / 5.27 ms) to ≈0.12–0.64 ms
std, now dominated by sender, ALSA sequencer and callback-wake jitter; the one larger value
(0.64 ms, light active 2) coincides with a 6.2 ms callback gap during Save/Open. Callback cost
is unchanged within run-to-run variation (dense p50/p99 overlap between builds). No xruns in
any run.

**Dense-run freezes (both builds, not D06-specific, cause not established).** 4 of 8 dense
runs stopped updating stats and the recording after 8–38 s (d06-dense-idle-1,
d06-dense-active-1, base-dense-idle-1, base-dense-idle-2); the app stayed alive until the
script ended it. A separate 20 s check of the same setup ran normally while llvmpipe used
~6 threads at 20–30 % each. These are Xvfb software-rendering sessions; whether the audio
callback also stopped is not established (the UI thread writes the stats). Listed as a
follow-up; partial runs are excluded from conclusions above.

Offline correctness does not establish physical controller latency: the controller-to-sound
delay is period + 64 frames + device output latency, which the app does not measure.

## Listening files and walkthrough

Offline renders (production `Timeline`, `render_midi.rs`, 48 kHz): hashes identical for
host buffers of 1, 64, 256 frames and irregular 1–700:

| File | Patch / performance | Length | Active (>-50 dBFS, 50 ms windows) | Peak | Hash |
|---|---|---|---|---|---|
| lead.wav | expressive/lead, lead.events | 34.5 s | 32.9 s | -4.8 dBFS | e461712dc8467f41 |
| keys.wav | expressive/split, keys.events | 31.0 s | 31.0 s | -1.4 dBFS | e4921f40415a127d |
| timing.wav | expressive/split, timing.events | ≈23 s | (see bundle) | | |
| timing-pre-d06-emulation.wav | same, events quantized to 256-frame callback starts | | | | |

Intentional silence: each file starts 0.5 s before the first note; the last 3–4 s are release,
echo and reverb tails. `lead` has vibrato (wheel) at 1.5–5 s and 17–21 s, bends at 10.8–15 s,
grace bends 21.8–25 s. `keys` has pedalled chords, repeats, overlaps (0–13 s), the channel-2
bass with glides and an octave bend dive (13.5–18.4 s), wheel on the keys' filter (19.5–22.8 s).

Walkthrough `walkthrough.mp4` (56 s, real app on Xvfb 1440×900, A-light): Keys and Bass by
Channel played by the scripted virtual controller; both MIDI In advanced areas (Bend, Channel);
the bass keyboard switched to ALL (a channel-1 chord then layers on both) and undone. The sound
track is the app's own recording of its output (Perform > Record), placed at the Record click
(offset from `drive.log`), not offline audio. Screenshots: `img/walkthrough/`, and
`img/{1440x900,1280x800}-{light,dark}/` (faces, bass advanced, bass drawer).

## Compatibility

Old patches: unchanged sound (hashes); `midi.in` gains params `bend` (default 2) and `channel`
(default ALL) and output `wheel`, so existing patches layer as before. Undo/save: ordinary
params and cables. D05: retry/fresh graphs rebuild keyboards (bend/wheel at rest); CC 1 still
reaches mappings. Behaviour changes, deliberate: CC 120/123 end only their channel; controller
disconnect no longer ends a running preview; the MIDI In face shows a second line; the
Velocity jack reads "Vel"; live MIDI has a constant one-period delay instead of 0–1 period
jitter. Plugin automation: not in scope.

## Limits

Unverified on hardware: physical controller feel/latency, unplug/replug, listening. No MPE,
aftertouch, RPN, 14-bit wheel, split zones, multiple controllers, host note ids. Control-thread
messages (knobs, mapped CCs, graphs) are not sample-timestamped (D08). Tail length for a host
is not reported (D07). The legacy `kabl` binary (crates/standalone) keeps its old block loop;
the product app is `kabl-ui`. The dense-run freeze above.

## Follow-ups (not implemented)

- Dense patch + Xvfb freezes in both builds: capture thread stacks (needs ptrace permission or
  running under gdb) and check whether the audio callback also stops.
- `MAX_HELD` (32) and 64 changes/voice/block are bounds; report when exceeded in stats.
- The MIDI In first summary line can still overflow its face for LEGATO · LAST · glide (pre-D06).
