# Unison to 16 voices (osc.va)

`osc.va` plays 1 to 16 band-limited oscillators per voice. Every new control is neutral at its default, so a patch with unison up to 4 sounds exactly as before.

## Controls

| Param | Range | Default | What it does |
|---|---|---|---|
| `unison` | 1 to 16 | 1 | Oscillators per voice. Drawn as a knob on the face. |
| `detune` | 0 to 100 ct | 15 | Spread from the lowest to the highest oscillator. |
| `curve` | 0 to 100 % | 0 | Detune distribution. 0 is even steps; 100 keeps the outermost oscillators and pulls the inner ones toward the centre (power 3), the shape of a classic supersaw. |
| `width` | 0 to 100 % | 0 | Equal-power pan of the stack, lowest oscillator left, highest right. 0 is mono. |
| `blend` | 0 to 100 % | 50 | Centre against side oscillators (centre = the one or two nearest the middle). 50 is equal, 0 centre only, 100 sides only. No effect below 3 oscillators. |
| `restart` | 0 or 1 | 0 | What a `sync` edge does to the stack: 0 restarts every oscillator at phase 0 (hard sync), 1 restarts each at its own start phase so a retriggered stack never lines up. |

New output `out_r`: the right channel. `out` is the whole stack at `width` 0 and the left channel otherwise; at `width` 0 `out_r` is a copy of `out`. To keep the stack stereo past the oscillator, wire both outputs and double the chain after it.

Start phases are fixed per oscillator (the original four, then golden-ratio steps), not random per note: the module has no note-on signal. Oscillators run free; `restart` 1 with a gate on `sync` retriggers the stack.

## Decisions

- **Level law: equal power.** Each oscillator plays at `weight / sqrt(sum of weight^2)`. With equal weights that is the old `1/sqrt(N)`. Measured RMS (saw 220 Hz, 30 ct) stays within 1.5 dB from 1 to 16 voices at blend 0, 50 and 100; 16 voices sit about 1.4 dB above one, because detuned saws are partly correlated.
- Unison up to 4 keeps its old position table, order and start phases; `curve` 0, `width` 0, `blend` 50 take the old code paths. Counts above 4 use even steps.
- Phase step is clamped to 0.5 (Nyquist). Only a pitch above half the sample rate reaches it.
- No schema change: the new params default for saved patches that lack them.

## Evidence

- **Digest:** `render_digest` (24 s render of every patch under `patches/`, FNV of the sample bits) on `origin/master` and on this branch: all patches identical (`evidence/digest-master-patches.txt`, `evidence/digest-unison-patches.txt`). Baseline made by building master's `osc_va.rs` in the same tree.
- **Alias** (`cargo test -p kabl-modules --test osc_unison -- --nocapture`, alias power relative to a full-scale sine, 16 voices against one oscillator, no detune): 48 kHz at 440 / 1760 / 3520 Hz: -41.8 / -40.6 / -35.3 dB against -42.5 / -35.6 / -31.6; 96 kHz: -63.0 / -62.0 / -57.4 dB against -63.6 / -58.4 / -54.5. Sixteen voices never alias more than one oscillator by more than 3 dB.
- **Numeric safety:** all outputs finite at 44.1, 96 and 192 kHz with every control at an extreme and a pitch at 19 kHz plus 36 semitones.
- **CPU** (`bench_sources`, one voice, 48 kHz, one core pinned): 1 voice 64.7 ns/sample (0.31 % of a core), 4 voices 69.4 ns (0.33 %), 8 voices 95.1 ns (0.46 %), 16 voices 185.6 ns (0.89 %), 16 voices with width 194.4 ns (0.93 %). Eight voices of polyphony at 16 unison cost about 7 % of a core.
- **Clips** (`clips/`, FLAC and 320 kbps mp3, rendered offline by `cargo run --release -p kabl-modules --example unison_clips -- docs/unison/clips`):
  - `unison-1-4-8-16`: the same A minor 9 chord at 1, 4, 8 and 16 voices, 40 ct detune. Listen for the stack thickening and the beating getting denser, with the loudness staying put.
  - `supersaw-curve-width-blend`: 16 voices. 0 to 4 s `curve` rises (the outer voices pull away from a tight core), 4 to 8 s `width` opens (the chord spreads across the field), 8 to 10 s `blend` swings from equal to sides only (the bright, hollow middle) and back.

## Face

`osc.va` now has four jacks; its face keeps them in one row. Unison is a knob, so the sixteen steps do not widen the face; `curve`, `width`, `blend` and `restart` sit in the advanced area. The live display already shows the stack because it draws the output cycle.
