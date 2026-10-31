# Handover: macro defects 2 and the macro redesign pilot

Branch `claude/groove-macro-defects-2` (from origin/master 6e92737, #68 merged). No code changed yet; this file is the state. Rules: no stash, no merge, `cargo clean` at the end, draft PRs only, fmt + clippy (`bash .github/scripts/clippy.sh`), audio_defects with no allow entries, REAPER/standalone on `kabl_agents`. Shell tip: the worktree guard refuses compound git commands and heredocs; use the Write tool for scripts and plain single commands.

## Status (third session)

- Loading Dock 6/5 dB vs 0.3 dB: both true at different points. #68 lowers the trim gain by 6.4 dB (Build) and 4.8 dB (Drop) (-9 dB per Sweep, +2.2 per Pump). With the music `compressor` bypassed (`PP_SET=compressor.ratio=1`) the cue rms drops Build -2.7 dB, Drop -4.7 dB (Intro 0.0). With it in the chain (as shipped) the compressor evens the level out: Build -0.2 dB, Drop -0.3 dB.
- Click reproduced: Corner Store, `PP_SET=macro.m2=0.5714` with `presets_groove clips patches OUT 1` gives CLICK at 3.748 s (the audit's m2-s4/s5; s6/s7 at 7.637 s). Not the cause: env attack (5 and 20 ms keep it), unison, drive and ladder drive_db. Cleared by: `vca.gain=1`, `env.adsr.release_ms=300`, `filter.ladder.resonance` 0 or 0.3 (not 0.1), main/all waveform sine or square. Changing the ladder cutoff moves it to other times, so the scanner window context decides. No engine fault shown: `seq`, `env.adsr` and `osc.va` read clean. Open: take a tap of the bus before the ladder (a probe in `render`) to see if the step is upstream of the ladder; if it is only a band-limited saw edge in a quiet window, it is a scanner/design question for Kosta, not a module bug.
- Redesign pilot: not started.

## Status (second session)

- Loading Dock re-level: NOT NEEDED. The trim sits before `crush` and the music `comp`, which absorbs it. Per cue (peak / rms dBFS, float, 30 s clip): before #68 Intro -11.8/-26.2, Build -13.4/-25.6, Gap -15.2/-25.9, Drop -13.2/-25.4, Break -13.5/-25.7; after #68 (= now, no change made) -11.8/-26.2, -13.6/-25.8, -15.1/-25.9, -13.5/-25.7, -13.7/-25.8. Macro audit steps: rms within 0.6 dB before vs after, peak worst now -1.5 (Pump 0) and -2.0 (Sweep). The "lost 5-6 dB" of the trim is not heard.
- Click cause: NOT found. Corner Store clip at Filter 0.72 (`PP_SET=macro.m2=0.72`, group 1) gives HF_BURST at 3.965, 8.385, 12.82 and no CLICK in the stored phrase. `vca.gain=1` clears all flags; `env.adsr.attack_ms=20` and `filter.ladder.cutoff_hz=18000` add CLICK 5.695; `unison=1`, `comp.ratio=1` change nothing. `seq` is sample-accurate (reviewed `seq.rs` process), `env.adsr` attacks from the current level (no reset). Next: reproduce the audit CLICK itself (macro_audit render, not the clip) and bisect there; build the module chain test only after.
- Redesign pilot: not started.
- Tools: `scratch/ld/` (untracked) has `cues.py`, `steps.py`, `probe_cs.py`; `presets_groove clips` takes a numeric group (1 = first eight), not a slug; the filter macro of Corner Store is `macro.m2`.

## 1. Osc phase check (Fault Line and Corner Store Filter "clicks")

Findings so far:
- `osc.va` has no gate input and no reset. Its phase restarts only on the `sync` input (`restart` param), and the groove bass never wires `sync`. So the oscillator is free-running: a new note does not reset its phase. (`crates/modules/src/builtins/osc_va.rs`; `seq_part` in `presets_groove.rs` wires `seq.pitch` to `pitch` only.)
- Corner Store, filter macro 0.72, flagged spike at 3.748 s and 7.638 s. Muting `mixer.level1` (the bass in the bus) removes it; muting the drums, chords or played voice does not. Longer attack, `unison=1`, a 1.5 ms pitch slew do not remove it. `env.adsr.release_ms=120` removes it (hides it).
- Waveform swap on the whole patch (`PP_SET=osc.va.waveform=N`): sine (0) and triangle (1) STILL flag a spike at 3.74 s (and 3.184, 7.906, 10.406); square (3) removes it (leaves 1.245). So it is likely NOT the saw's reset edge, as I wrote in `MACRO-DEFECTS.md`; that sentence should be corrected.
- The sample dump at the flagged sample (sweep clip, 3.7486 s) shows a 4-sample drop 0.25 -> -0.30 at the start of a bass note, preceded by a ringing tail (rms before 0.11, after 0.16). The spike sits about 1.3 ms before the swung step onset (step 23 of 108 bpm, swing 40: 3.7499 s).
- Next step to pin it: build the bass chain alone in a test (`seq` -> `osc.va` -> `filter.ladder` open -> `vca` with `env.adsr`) and play two notes legato (gate held, pitch change) and retriggered; look at the first difference at the pitch change. Candidates: the pitch jump itself (a frequency jump in a free-running osc is a slope change), the `env.adsr` retrigger from a non-zero level, the `vca` gain step, the sub oscillator (square: it removed the spike). Test script idea: `crates/modules/tests/` with `registry::create`, like `drum_block_edge.rs`. If a module resets or steps, fix in the engine with a regression test and report which presets' renders change and by how much. Do NOT make the scanner ignore it.
- Tools: `PP_SET="kind.param=value,..."` with `presets_groove clips patches OUT GROUP` (PP_F32=1), my scripts were `mute2.py` style (render, `audio_defects.clicks`). The macro audit tools are in master now (#66 merged or in #68: `crates/ui/examples/macro_audit.rs`, `docs/presets/scripts/macro_audit.py`).

## 2. Loading Dock re-level (not done)

#68 trims Loading Dock after the filter: -9 dB per unit of Sweep and +2.2 dB per unit of Pump (base `9.0 + 9.0*0.12 - 2.2*0.5`), so Build (Sweep 0.9) and Drop (0.78) lost about 6 and 5 dB. Task: restore their loudness, every macro position and cue still <= -1 dBFS; report loudness and peak per cue (Intro 0-4 s, Build 4-10, Gap 10-12, Drop 12-20, Break 20-30 of the 30 s clip) before #68 (`git archive 6e92737^1 patches`), after #68, now.
Why the peaks exist: only the first beat of a run (0.5003 s, 9 samples) is over; later beats are 12 dB lower. Idea (untested): drop the macro slopes (base back to 9.0) and add a soft start: an `env.adsr` (attack 0, decay about 250 ms, sustain 0) triggered by the clock `reset` pulse at Run, routed into `trim.gain_db` with amount `-4/84`, so only the first beat is about 4 dB lower. Check that the clock emits `reset` at Run (the seqs rely on it), that the 8 audit steps of Sweep and Pump are <= -1, and that the second beat (0.48 s later) is not dented. If a routed env into a `gain` param is not allowed, route it into the music compressor makeup or a second gain stage.

## 3. Redesign pilot brief (verbatim from the overseer, queued after the above)

Kosta: macros are not separated enough; they double up on the same sound change. Macros must be 4 clearly separate, audibly different, expressive ways to change the sound. They are NOT bound to a fixed list of roles ("Filter", "Room" by habit): each macro comes from what makes THAT patch interesting. A macro may move several params at once if that makes one expressive gesture; fun and free, not "this does exactly X".
Pilot: Copper Mile and Brick Dust only (Kosta listened to these). New branch claude/macro-redesign-pilot from origin/master.
Per preset:
1. Listen-through first (describe in words what the patch is and what is interesting to play with). Propose 4 macros: name, what you hear moving 0->1, what it moves underneath. Write that in docs/presets/MACRO-REDESIGN.md.
2. Build them. Check with macro_audit: each macro audible over its whole travel (no dead third, no switch-like first step), peaks <= -1 dBFS at all 8 positions, no clicks; and pairwise the 4 difference signatures must be clearly different (report the similarity matrix; flag any pair that measures alike). Measures are only a sanity check: the ear decides.
3. Default (stored) sound stays as Kosta approved it; report dB diff of the default render.
Tour per preset: old macros 0->1->0 then new ones, start times listed. Also send me one line per macro of the new design.
Usual checks (audio_defects no allow, REAPER/standalone on kabl_agents, fmt, clippy). Push, DRAFT PR, report PR number + tour path. Do not merge. No git stash. cargo clean at end. Checkpoint at 50% (Kosta's number).

Notes for the redesign: the generator is `crates/ui/examples/presets_groove.rs` (`groove(Spec)`, `T::` targets in `Spec.targets`, the fourth macro is always the space macro in `finish_drums`; new targets need new `T` variants or a custom macro loop). Audit data from #66/#68 (`docs/presets/MACRO-AUDIT-GROOVE.md`): Copper Mile Filter weak (top flat), Phase waveform-only, Pump switch-like (first step does 80%), Room ok; Brick Dust Filter weak, Dirt weak (saturated drive), Pump switch-like, Room weak. The new macros should overlap in none of the difference signatures (cosine matrix from `macro_audit.py`'s `vec`).
