# Pad macro slice (S3-pad): handover

Branch `claude/macros-pad` (worktree `.worktrees/living-rack`). Builder: `tools/examples/presets_pad.rs`.
Prompt: `.kabl-agent/PROMPT.md` (git-ignored). Overseer: "Kabl overseer [e0ae55]".

## State
- All 13 presets redesigned (commit 9ec968ee builder, 425cfc5c patches regen). No PR yet, no doc yet.
- Helpers ported into the pad builder: `span`, `span_hi`, `span_lh` (derives the stored position from
  lo / stored value / hi and stores the macro there), `rise`, `routed`, `comp` (additive gain slope on
  the voice's gain stage, keeps the stored sound), `fixed_vibrato`, `PP_LAX=1` (lists every range
  problem instead of panicking). `finish("=0.5", ...)` means the echo/room are fixed at that stored
  macro position (no space macro).
- The -1 dB limiter was removed (Kosta's rule #53: no limiter; gain staging only). Audit with the
  limiter gone: every step and combination render is at -1.2 dBFS or lower
  (`/home/stcksmsh/.claude-alt/jobs/0d8c28d4/tmp/peaks.py <report>` prints them).
- Macros (stored positions are set by the helpers):
  - Fog Bank: Arrival, Haze, Drift (pitch sway only), Space
  - Deep Field: Swell, Sub, Shimmer, Space
  - Cotton Mill: Motion (PWM), Chorus, Warmth (cutoff), Beat (saw detune)
  - Picture Window: Spread, Sparkle, Chorus, Air (new `eq` high shelf at flat)
  - Drive-In: Sway, Wash, Weight, Pulse (new trance gate on the host clock, stored 0, press Run)
  - Dry Cleaner: Sweep, Phase, Grit, Snap
  - Concrete Cloud: Weight, Squash, Crush, Tail
  - Static Garden: Static, Wander (pitch drift, stored 0; filter walk now fixed), Metal, Space
  - Glacier: Frost, Width, Swell, Space (echo max .30, stored .70)
  - Pump House: Pump, Weight, Wide (chorus), Open (Heat dropped, drive fixed)
  - Tin Voice: Vowel (formant `hold` 1: default change), Chatter (S&H, stored 0), Shift, Grit
  - Rooftop: Swell, Section, Octave (new third osc, stored 0), Chorus
  - Tidepool: Drift, Steps (S&H pitch, stored 0), Tone, Space
- Default changes to state in the PR: Tin Voice (hold 1, trim 14 to 23), Deep Field trim 11 to 15
  (patch gain), Picture Window trim 10 to 11, Rooftop trim 9 to 11, Glacier trim 9 to 12, Tidepool trim
  9 to 11; Cotton Mill / Picture Window / Drive-In / Dry Cleaner / Concrete Cloud / Rooftop / Glacier / Pump
  House / Static Garden lost their old Cutoff/Room/Space macros (fixed at the old stored value); Fog Bank
  Drift no longer moves the filter (walk fixed at its old stored depth).
- Tours: `docs/presets/clips/macros-pad/` (git-ignored), rendered with
  `bash /home/stcksmsh/.claude-alt/jobs/0d8c28d4/tmp/run_tours.sh <slugs>` (copy of that script: per slug
  `MACRO_TOUR_BPM=124 macro_audit patches <phrases.json> <renders> <slug>` then `macro_tour_ab.py`).
  Phrases json: `presets_pad phrases <file>`. Listen-first three: Tin Voice, Drive-In, Rooftop.

## Kosta's verdict on the listen-first tours (and two new rules) still TO DO
- Tin Voice: Vowel good, Chatter ok, Shift ok, Grit could be better (idea: add a bitcrusher after the
  formant, Grit = drive + crush mix + bits; bolder).
- Drive-In: Sway more pronounced; Wash sounds ominous: rename to what it does (it is the chorus:
  call it Chorus); Weight good but more pronounced; Pulse ok. Preset rename wanted: propose one (name must
  be distinct in the bank; a rename means a new slug, removal of the old patch folder, and updating the
  FACTORY entries in `crates/ui/tests/library.rs` and docs) and tell the overseer before doing it.
- Rooftop: Swell ok, Section more pronounced, Octave good, Chorus more pronounced.
- Rule 1 LEVEL: a non-level macro (tone, motion, space, texture) keeps loudness roughly constant over
  its travel: compensate with `comp` (slope = loudness at 1 minus loudness at 0, from the audit steps;
  `sumry.py <report>` prints `loud 0 / 1`). Level-type macros (swell, pulse, arrival, tail, Pump) may move
  level on purpose.
- Rule 2 BOLDER: push every range further (chorus mixes, detune, drive, crush, tone floors). Level
  compensation and the combination check keep it safe. Peaks stay <= -1 dBFS.
- The overseer will tell when the `level` flag of `macro_audit` is on master: merge origin/master and run
  it. Until then measure loudness 0 vs 1 yourself.
- Then: re-render the 3 listen-first tours and send `TOURS <paths>`.

## Still to do
1. The two rules and the feedback above; re-run the audit with combination for all 13
   (`macro_audit patches phrases.json <renders> <13 slugs>` then `macro_audit.py --category Pad ...`).
2. `docs/presets/MACRO-REDESIGN-PAD.md`: one table per preset (macro, stored, 0->1, what it moves),
   flags (shared-gain `duplicate` flags for Deep Field Sub/Shimmer, Drive-In Weight/Pulse and Pump
   House are artefacts of `comp` sharing the gain stage; Pulse `jump` is a gate), combination results,
   count justifications (all presets have 4), default changes.
3. `cargo fmt`, `cargo +1.99.0 clippy --workspace --all-targets -- -D warnings`, workspace tests to a
   file (`grep '^test result|FAILED'`; the factory level test is `crates/ui/tests/factory_bank.rs`).
4. Regen patches (`presets_pad write patches`) as a tool-output-only commit; draft PR (attribution lines,
   `gh api` for the body), wait for all checks, send `DONE <PR#> <commit>` + three lines + doc path.

Never merge, never stack PRs, no limiter, no audio on the default sink.
