# Handoff — for the next blank-context agent

Read this, then `docs/STATUS.md` (top section), the last entries of `docs/decisions.md` and
`docs/rack-migration/README.md`. Trust code and git over older docs.

## Where things are

- Branch `master`, local only (not pushed). Rack UI migration done, review prep done
  (`1e402cf` lane edge fix, then fresh screenshots and a walkthrough video); stopped for Kosta's
  review of the migrated production rack. Before it: modulation slice (`c02fc89`, docs `fc4e2de`).
- Workflow: commit small working changes straight to `master`; no branches, PRs or pushes
  unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use. Don't `cargo fmt` the whole
  crate: it reformats the untouched `rev2_proto` / `rev2_env_ab` examples; use `rustfmt` on
  the files you changed.

Launch (real audio + MIDI; first non-"Midi Through" port):

    cargo run --release -p kabl-ui -- --patch patches/reference            # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/reference --size 1280x800
    cargo run --release -p kabl-ui -- --patch patches/crowded

## Accepted decisions (owner-confirmed)

Visual / layout (built now): A-light / A-dark core modules; illustrated skins with
`labels_on_art` default false; user-selected primary controls, advanced controls expand in
place; Hidden cables use the stable rack; expansion pushes neighbours by default, float is a
setting; 1440×900 default, 1280×800 usable minimum; compact layout later and optional.

Modulation (built, unchanged): any knob accepts routes; cable, ring, lane/dot, badge and
drawer row are one route (amount signed, invert, bypass); +25 % on drop; body = base, ring =
selected route; inspected knob shows one lane per route (single source too); collapsed
multi-source knobs show display-only rings; removing the selected route leaves none selected;
vertical drag, Shift ×0.1, Escape cancels with no undo entry; taper-space sum, one clamp,
block rate; envelope timing CONT (default) / KEY per envelope.

## What the migration added (details in decisions.md, "Rack UI migration")

- `crates/ui/src/rack.rs`: pure layout (rows packed from stored positions, faces, advanced
  area, push/float, snap) with unit tests. `theme.rs`: palettes, chrome, fonts.
- `lib.rs`: rack rendering, toolbar (themes, cable views, zoom, View menu, save/load, drawer
  toggle), choose mode, module menu, float layer, drop targeting. `routing.rs`: same
  modulation logic, A-style drawing, zoom-aware geometry.
- Face choice = `face.<param>` params (grouped op, no audio rebuild, no schema change).
  `ModuleInfo.advanced` declares defaults. Mixer params renamed with legacy compatibility.
- Source lanes: inspecting a knob (or adding a route to it) pans, once the pointer is up, just
  enough to show the whole lane disk; opening the drawer too. Dots outside the canvas catch no
  presses unless mid-drag. Selected drawer row text is white.
- Skin struct: dark art, `labels_on_art`, `art_ink`. Skins on by default; no built-in has one
  (osc.va demo dropped, owner); the renderer is covered by a test skin in `rack.rs`.

## Verification evidence

- `cargo test --workspace`: 193 pass (3 ignored = fixture/script writers). Clippy clean.
- `crates/ui/tests/interaction.rs`: modulation suite plus rack tests (faces, push/float,
  off-face reveal, gestures after zoom/pan, view changes don't touch patch/audio, face params
  render bit-identical, drawer keeps selection visible, lanes at the drawer edge and bottom-left
  corner pan into view and still drag), both sizes.
- Real binary + xdotool: `docs/rack-migration/closeout-real-x.sh` (MATCH at both sizes, rerun
  on `1e402cf`); screenshot tours `docs/rack-migration/scripts/*.txt` via `drive.py` (Xvfb,
  `KABL_HITS_FILE`), rerun on the final build: `docs/rack-migration/img/` (old osc.va demo
  shots moved to `img/historical/`). `drive.py` has `pan` for dragging bare rack.
- Screen + audio: `docs/rack-migration/walkthrough.mp4` from `record-walkthrough.sh` (null
  sink `kabl_rec`, Midi Through, `target/slice-av/chords.mid`). Level-checked only.
- `patches/crowded`: its off-face route now targets VCA #9 Response (Timing moved onto the
  ADSR face, so the old env → Timing route was no longer off-face).
- Earlier audio evidence (modulation slice) still applies; the audio path is unchanged:
  `docs/modulation-slice/record-av.sh`, renders, `bench_reference` (not rerun: DSP unchanged).

## Awaiting Kosta

Review of the migrated rack. Kosta heard the remote recordings/renders; nobody has used the
builds by hand: grab feel (dots/ring/body, now also zoomed), his display and scale factor, his
MIDI controller and sound card are unverified. If he reports a concrete problem, fix it within
this scope. Owner answers after the first review: Patchbay stays removed; ADSR Timing on the default
face; jack cables are removed by pulling the plug (what VCV Rack / Voltage Modular do); skins
on by default with the osc.va demo skin dropped; no real art needed.

## Unresolved / known limits

Block-rate modulation; no hysteresis on stepped destinations; pitch full scale ±60 st; state
carry O(modules²) per swap; jack inputs take one cable; engine output quiet; Pi 4 unmeasured;
drawer sliders lack Shift/Escape; selector routes show no reachable-options bracket; lanes
pan into view only on inspection (a later user pan/zoom can still clip them); no
`labels_on_art` contrast warning; no module ships a skin yet; walkthrough audio not yet heard
by anyone; `docs/modulation-slice/xdotool-walkthrough.sh` uses
headless coordinates and is stale for the rack (use `drive.py`).

## Not in scope

Sequencers, effects, new synthesis, full rich LFO, plugin work, compact layout, framework
change.
