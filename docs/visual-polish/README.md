# Visual polish pass: audit, proposal, mockups

Branch `claude/visual-polish-proposal`. Audit and proposal only: **no product change**, no code under
`crates/`. Kosta chooses before anything is built.

| Document | What it is |
|---|---|
| [`AUDIT.md`](AUDIT.md) | Ranked problems against the Vital / Pigments bar, with the captures behind them |
| [`PROPOSAL.md`](PROPOSAL.md) | The integration language (panel, well, hardware), two contrasting options, before and after mockups |
| [`BUILD.md`](BUILD.md) | What changes in the theme model, face kit and faces; order; split across agents; staged delivery |

## Media

- `media/faces/faces-<theme>-<mode>-z100.webp` and `-z50.webp`: all 51 module kinds, 8 contact sheets
  (theme `satin` or `lacquer`, `light` or `dark`; 100% and the app's third zoom-out step, 58%).
- `media/scenes/<size>-<scene>-<shot>.webp`: 2x2 sheets (A light, A dark, B light, B dark) at 1440x900 and
  1180x680: simple and dense rack (routing drawer open and closed), Perform, cable pattern editor, Sounds, Save As,
  Diagnostics, settings menu, About, drum grid, wavetable editor.
- `media/mockups/board-<theme>-<mode>.png`: current captured faces against Option 1 and Option 2 (drawn);
  `closeup-*.png`: 2x close-ups.

The plugin editor was not captured in a host in this task (same renderer, same 1180x680 size).

## Decisions for Kosta

1. **Option for the well.** Option 1 Seated (hardware depth), Option 2 Tinted (flat, hue-continuous), or the
   recommended split: 1 for theme A, 2 for theme B. Or one for both.
2. **Remove the printed `Edit face`** from every face (hover and context menu only). Recommended yes.
3. **Left-aligned titles** with the `+N` chip on the same row. Recommended yes.
4. **Jacks move** (inputs about 11 px down, outputs about 3 px) onto the new port tray rows. Same kind of move
   as the one already accepted for faces with displays. Recommended yes.
5. **Labels may wrap to two lines** on 210 px faces (`Cutoff CV`, `Res CV`).
6. **Staged or all at once** (`BUILD.md`, "Staged delivery"). Stage 1 alone (5 days) fixes the two worst
   problems.
7. **Empty faces** (Comparator, Logic, Clock Divider, Ring Modulator, Sample & Hold, Quantizer, Random, Output):
   fill with larger controls (proposed), or change those faces' width. Width change would move neighbours in
   saved racks and is not proposed.

## Reproduce

```
cargo build --release -p kabl-ui --bin kabl-ui
python3 docs/visual-polish/scripts/capture_faces.py OUT/faces 4          # 51 kinds x 2 themes x 2 modes x 2 zooms
python3 docs/visual-polish/scripts/capture_scenes.py OUT/scenes 4        # chrome scenes, two sizes
python3 docs/visual-polish/scripts/sheets.py faces OUT/faces OUT/sheets
python3 docs/visual-polish/scripts/sheets.py scenes OUT/scenes OUT/sheets2
python3 docs/visual-polish/mockups/make_mockups.py OUT/faces OUT/mockups  # needs inkscape
```

The capture scripts run the release editor under Xvfb with the sink set to `kabl_agents`, so nothing plays on a
real device. They use throwaway config folders and `KABL_THEME` so the user's settings are never read or written.
Capture runtime on this machine: about 20 minutes for faces, 6 minutes for scenes (four displays in parallel).
