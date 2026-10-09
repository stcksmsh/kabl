# Factory Performance Bank

Browse Sounds, Sequences or Performances in the existing category menu. Combine purpose with Factory/Your Sounds and search by name, musical role, mood, tempo or tonal center. The twelve curated entries have `curated` tags and embedded playing guides. Opening one prepares Perform automatically; sequenced entries open stopped. Existing factory content remains available under its established identity and role category.

| Factory ID suffix | Name | Purpose | Prepared controls |
|---|---|---|---|
| palette/keyboard-bass | Round Keyboard Bass | Playable bass | Tone, Weight, Shape, Room, Release, Level |
| palette/lead | Singing Lead | Legato lead | Tone, Vibrato, Bite, Echo, glide, level |
| palette/keys | Glass Keys | Pluck/keys | Tone, Weight, Shape, Room, release, level |
| palette/pad | Evolving Pad | Atmospheric pad | Warmth, Evolve, Air, Space, attack, release, level |
| palette/strings | Ensemble Strings | Bowed ensemble | Bright, Ensemble, Bow, Space, chorus, release, level |
| palette/breath | Breath | Airy texture | Breath, Gust, Color, Space, width, level |
| palette/bass | Sequence Bass | Bass pulse | Run/Stop, four macros, Line/Lift/Hold/Rest banks, transpose, tempo |
| interlocking | Interlocking Sequences | Interlocking arp | Run/Stop, Glow/Motion/Length/Air, two real bank cards, tempo |
| echo | Echo Sequence | Probabilistic motion | Run/Stop, Glow/Motion/Length/Echo, two real bank cards, tempo |
| palette/progression | Slow Horizons | Slow harmonic progression | Run/Stop, Glow/Motion/Bloom/Space, Home/Lift/Descent/Rest, tempo |
| composition | Composition | Complete sectional performance | Run/Stop, Energy/Motion/Space/Glow, two bank cards, five section cues, lead |
| sound-palette | Sound Palette Piece | Complete layered performance | Run/Stop, Energy/Motion/Bright/Space, five section cues, keyboard layer faders |

All twelve use original synthesis and existing modules. Six voices use keyboard notes; sequenced content uses editable native sequencers. Macro travel is 0–1 with authored modulation amounts, not a separate DSP control model. CC20–23 on MIDI channel 1 control the four macros with existing soft takeover. The guides name appropriate keyboard registers, expression and actual sections. Use the existing question-mark control to inspect each macro's concrete targets. Bank changes queue for the next bar; queued changes never survive a new load. In Host mode REAPER owns Run/Stop; Free mode uses the clock buttons.

Nine entries reuse existing IDs and graphs. New IDs fill the missing standalone keyboard bass, plucked keys and slow harmonic progression. Interlocking/Echo append macros and editable banks without renumbering prior modules; Composition now gates its formerly constant pad so stopped loads remain silent. Existing saved projects keep their embedded old state; factory improvements do not mutate them. Embedded guides, pins, banks and cues survive complete-state save/recall independently of later library edits.

## Reproduction

Toolchain: Rust/Cargo 1.93.0, Linux x86_64. No new dependencies. `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=4` keeps temporary build size bounded; release uses the workspace's existing LTO/panic profile.

```sh
cargo test -p kabl-ui --test interlocking write_interlocking_patch -- --ignored
cargo test -p kabl-ui --test echo write_echo_patch -- --ignored
cargo test -p kabl-ui --test composition write_composition_patch -- --ignored
cargo test -p kabl-ui --test factory_bank write_bank -- --ignored
cargo test -p kabl-ui --test expressive write_expressive_patches -- --ignored
cargo test -p kabl-ui --test library write_factory_library -- --ignored
cargo test --workspace -- --test-threads=1
cargo test -p kabl-ui --test factory_bank render_bank -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p kabl-ui --bin kabl-ui -p kabl-clap --lib
KABL_BANK_BINARY=target/release/kabl-ui python3 docs/factory-performance-bank/scripts/capture.py
python3 docs/factory-performance-bank/scripts/host.py
```

The general library writer also regenerates Init and recipe logs; timestamp-only changes to those unrelated definitions are excluded from this submission. Writer order above ends with the curated metadata overlay and carries guides into the expressive lead derived from Singing Lead. The source builders and exact committed-state checks verify the musical definitions.

## Audio and Performance

Audio is synthesized through the production compiler/engine with scripted notes and runtime controls, not recorded reference music or prerecorded sequences. Every short preview uses a fresh eight-voice engine at 48 kHz on the 64-frame grid for 24 seconds: stopped/silent first second; Run/note-on at 1 s; note-off at 4 s; new notes at 5 s; next-bar bank launch at 6 s; note-off at 9 s; final note-on/off at 13/17 s when permitted before the 16 s stop. Macros move at 3/7 s; a later 19 s change occurs during release. Stop at 16 s clears all keys and clocks, followed by eight seconds of measured tail. The second render sets all macros to one at the control points. No post-render normalization or limiting is applied.

The 72-second `editable-performance` uses the actual Sound Palette graph, keyboard layers, bass/percussion sequencers, section cues and macro changes. Cue requests occur at 2/12/24/36/48 s, on next-bar boundaries. Additional key phrases start at 21/29/37/45/53 s and release at 25/33/41/49/57 s. Stop at 64 s leaves an eight-second tail. The complete editable patch accompanies the take; reproduction uses real sequencers and commands. This demonstrates several instrument voices and transitions inside one complete performance, rather than swapping prerecorded audio.

The long take explicitly changes Breath/Strings/Pad/Lead faders at each cue request: 2 s = 0.8/0/0/0; 12 s = 0.3/0.55/0/0; 24 s = 0.15/0.3/0.55/0; 36 s = 0/0.25/0.35/0.5; 48 s = 0.5/0.2/0.3/0. Lead echo send is 0.2 during Lift and zero otherwise. These runtime gestures make shared Strings, Pad and Singing Lead voices audible, not merely present in the graph. The saved patch remains fully editable; the test records the complete gesture score.

See `audio/` for previews, `evidence/audio/` for peak/RMS/activity/tail results, and the reproduction test for the exact event schedule. The silent GUI walkthrough is separate from audio. Source structure and audible preview files establish distinct candidate roles; metrics do not establish musical usefulness or listening approval. Kosta's listening and controller review remain pending.

## Delivery and Limits

The portable Linux owner bundle includes release standalone/CLAP binaries, the entire factory library, the editable performance, REAPER project, docs, media and notices. `launch.sh` uses bundled factory content and preserves an explicit user-library environment or normal native discovery. The explicit unavailable-library launcher uses a private profile and saved complete patch. REAPER is an installed external host, not bundled or globally modified.

Engineering checks, submission, merge and owner acceptance are separate. Physical controller feel, physical latency/xruns and production-beta approval remain unverified. This batch does not resolve ordinary host render nondeterminism, inherited full warm-up policy, dense-editor/backend limits or strict-profile assumptions. Exact heads, checks and artifact locations are indexed by REPORT and verification receipts.
