# Handoff — for the next blank-context agent

Read this, then `docs/STATUS.md` (top section) and the last entries of `docs/decisions.md`.
Trust code and git over older docs.

## Where things are

- Branch `master`, local only (not pushed). The rack UI migration is **closed**: Kosta
  reviewed it hands-on and approved it (decisions.md, "Rack migration closed"). Record:
  `docs/rack-migration/README.md` (screenshots, `walkthrough.mp4`, verification).
- Workflow: commit small working changes straight to `master`; no branches, PRs or pushes
  unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use. Don't `cargo fmt` the whole
  crate: it reformats the untouched `rev2_proto` / `rev2_env_ab` examples; use `rustfmt` on
  the files you changed.

Launch (real audio + MIDI):

    cargo run --release -p kabl-ui -- --patch patches/reference            # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/reference --size 1280x800
    cargo run --release -p kabl-ui -- --patch patches/crowded

## What exists (owner-approved)

- Production rack (`crates/ui`): `rack.rs` pure layout with tests, `theme.rs`, `lib.rs`
  rendering/toolbar/pan/zoom/drawer, `routing.rs` modulation. A-light / A-dark, All/Focus/
  Hidden cables, user-chosen faces (`face.<param>` params, no audio rebuild), push/float
  expansion, 50–200 % zoom, pull-the-plug cable moves, skin renderer (no built-in skin).
- Modulation: any knob takes routes (signed amount, invert, bypass); ring/lanes/drawer are one
  route; taper-space sum, one clamp, block rate; envelope timing CONT/KEY.
- Verification: `cargo test --workspace` 195 pass (3 ignored = fixture writers), clippy clean.
  Real-X tooling: `docs/rack-migration/drive.py` (+ `scripts/`), `closeout-real-x.sh`,
  `record-walkthrough.sh`.

## Sequencing — built, waiting for Kosta to try it

`clock` (BPM, 16th-note gate) and the 8-step `seq` module (pitch plus OFF/ON gate per step)
are both global rate. The demo is `cargo run --release -p kabl-ui -- --patch patches/sequence`.
The `decisions.md` entry "Shared clock + basic pitch/gate sequencing" lists what was built and
what was deliberately left out. Wait for Kosta's feedback before extending it.

## Accepted limitations

Block-rate modulation; no hysteresis on stepped destinations; pitch full scale ±60 st; state
carry O(modules²) per swap; one cable per jack input; quiet engine output; Pi 4 unmeasured;
drawer sliders lack Shift/Escape; selector routes show no reachable-options bracket; lanes pan
into view only on inspection; no `labels_on_art` contrast warning; no built-in skin;
`docs/modulation-slice/xdotool-walkthrough.sh` is stale for the rack (use `drive.py`).

## Not in scope

Effects, new synthesis, full rich LFO, plugin work, compact layout, framework change.
