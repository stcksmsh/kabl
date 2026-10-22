# Factory patch levels

Every factory patch is rendered the same way and held to one level target, so a preset never jumps in volume or clips the output when you switch to it. Preset authors: this page is the rule for new patches.

## The target

| | Value | Why |
|---|---|---|
| Peak | no sample above **-1 dBFS** | The output meter turns to CLIP above 0 dBFS, and a host or a later gain stage needs a little room. |
| Loudness | **-19 dBFS RMS**, within **+-5.5 dB** (aim for +-3) | -19 dBFS is where the bank's median already was, so most patches stay as they were and only outliers move. |

Loudness is the RMS of the 100 ms blocks above -50 dBFS (silent gaps and the stopped lead-in do not count) over a 24 s render at 48 kHz. Each patch is rendered the way it demonstrates itself: held chord for `keys` patches (notes C, E, G, B as in the bank evidence), and the clock run and sequences launched for the self-playing ones. The factory-bank render in `crates/ui/tests/factory_bank.rs` is the one source of this recipe.

`crowded` ("Crowded Rack", a study of dense cabling with resonant filters) is the reason the window is +-5.5 and not tighter: its peaks sit 22 dB over its loudness, so at -1.8 dBFS peak it is at -23.7 dBFS loudness. Patches with a normal crest (12 to 18 dB) should land within +-3 of the target.

## How to level a new patch

1. Render it: `cargo test --release -p kabl-ui --test factory_bank level_table -- --ignored --nocapture` prints `peak` and `loud` for every factory patch.
2. Change **level only**: the `gain_db` of a Gain module after the voice, or a Mixer level (a mixer level can only lower a signal, 0 to 1). Do not change the sound (filters, envelopes, effects mixes) to fix a level. A patch with a VCA straight to the output has no level knob: add a Gain module between the VCA and the output (see `recipes/pluck-to-pad`).
3. `cargo test -p kabl-ui --test factory_bank factory_levels_hold_the_target` fails the build when any factory patch clips or leaves the window. It renders every patch under `patches/`, so a new preset is checked as soon as it is added.
4. Refresh the golden hash of a patch whose level you changed (`KABL_WRITE_GOLDEN=1 cargo test -p kabl-engine --test golden_renders`, then keep only the lines of changed patches).

## What changed in this pass

Levels only: `gain_db` of the Gain stage, mixer levels, or (pluck-to-pad) one added Gain module. Sounds are the same, quieter or louder. The golden renders of these thirteen patches changed, by level only: `crowded`, `expressive/split`, `palette/bass`, `palette/breath`, `palette/keys`, `palette/pad`, `palette/progression`, `palette/strings`, `recipes/pluck-to-pad`, `sound-palette`, `sound-engines/demos/glass-bells`, `sound-engines/demos/imported-morph`, `sound-engines/demos/tine-keys`. `cue-macros`, `dx-keys`, `iron-bell` and `rust-bass` also changed level; they are not in the golden list. The patch format is unchanged; the table is dBFS.

`*` The two arpeggiator demos were measured silent before because the first measurement did not hold a chord; they were not changed.

| Patch | Peak before | Peak after | Loudness before | Loudness after |
|---|---|---|---|---|
| `composition` | -3.8 | -3.8 | -18.0 | -18.0 |
| `crowded` (changed) | 8.9 | -1.8 | -11.9 | -23.7 |
| `cue-macros` (changed) | -3.6 | -2.9 | -22.2 | -21.4 |
| `echo` | -6.3 | -6.3 | -19.5 | -19.5 |
| `expressive/lead` | -7.5 | -7.5 | -19.2 | -19.2 |
| `expressive/split` (changed) | -1.4 | -13.4 | -7.1 | -19.2 |
| `functional-cables/echo-throws` | -2.9 | -2.9 | -15.9 | -15.9 |
| `functional-cables/five-against-eight` | -4.1 | -4.1 | -17.2 | -17.2 |
| `functional-cables/macro-morph` | -3.0 | -3.0 | -17.4 | -17.4 |
| `functional-cables/morph-arc` | -3.8 | -3.8 | -18.1 | -18.1 |
| `init-keyboard` | -7.4 | -7.4 | -21.3 | -21.3 |
| `interlocking` | -7.2 | -7.2 | -18.7 | -18.7 |
| `palette/bass` (changed) | -6.1 | -11.1 | -13.9 | -18.9 |
| `palette/breath` (changed) | -5.3 | -2.3 | -22.6 | -19.7 |
| `palette/keyboard-bass` | -12.1 | -12.1 | -19.4 | -19.4 |
| `palette/keys` (changed) | -7.7 | -2.1 | -25.0 | -20.1 |
| `palette/lead` | -7.5 | -7.5 | -19.2 | -19.2 |
| `palette/pad` (changed) | -6.5 | -2.5 | -23.2 | -19.2 |
| `palette/perc` | -3.7 | -3.7 | -17.1 | -17.1 |
| `palette/progression` (changed) | -7.3 | -3.4 | -23.3 | -19.6 |
| `palette/strings` (changed) | -6.0 | -3.0 | -22.1 | -19.2 |
| `performance` | -1.0 | -1.0 | -16.8 | -16.8 |
| `recipes/filter-movement` | -6.7 | -6.7 | -19.6 | -19.6 |
| `recipes/pluck-to-pad` (changed) | -8.5 | -2.0 | -25.9 | -20.1 |
| `reference` | -6.9 | -6.9 | -21.9 | -21.9 |
| `sequence` | -8.5 | -8.5 | -16.2 | -16.2 |
| `sound-engines/demos/arp-cable` (changed) | silent* | -4.0 | silent* | -15.4 |
| `sound-engines/demos/arp-chord` (changed) | silent* | -7.3 | silent* | -20.0 |
| `sound-engines/demos/clock-logic` | -3.6 | -3.6 | -17.7 | -17.7 |
| `sound-engines/demos/dx-keys` (changed) | 0.1 | -1.6 | -20.1 | -21.8 |
| `sound-engines/demos/glass-bells` (changed) | -0.3 | -1.8 | -17.1 | -18.6 |
| `sound-engines/demos/imported-morph` (changed) | 6.1 | -5.9 | -6.7 | -18.3 |
| `sound-engines/demos/iron-bell` (changed) | 1.9 | -1.6 | -15.7 | -19.0 |
| `sound-engines/demos/ratchet-roll` | -7.2 | -7.2 | -18.0 | -18.0 |
| `sound-engines/demos/rising-pad` | -2.3 | -2.3 | -18.4 | -18.4 |
| `sound-engines/demos/rust-bass` (changed) | 4.2 | -5.8 | -8.9 | -18.8 |
| `sound-engines/demos/scale-walk` | -2.3 | -2.3 | -16.6 | -16.6 |
| `sound-engines/demos/stepped-sweep` | -3.5 | -3.5 | -16.7 | -16.7 |
| `sound-engines/demos/swing-seq` | -4.6 | -4.6 | -14.9 | -14.9 |
| `sound-engines/demos/tine-keys` (changed) | -0.3 | -1.8 | -17.1 | -18.5 |
| `sound-engines/demos/vowel-drift` | -3.0 | -3.0 | -19.0 | -19.0 |
| `sound-palette` (changed) | -6.5 | -2.5 | -23.8 | -20.1 |
