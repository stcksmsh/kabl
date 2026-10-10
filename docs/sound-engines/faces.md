# Faces of the sound-engine and utility modules in the editor

Every module added in PR #17, #19 and #20 was opened in the real release editor (`kabl-ui`, Xvfb,
metacity, scripted X input) and captured at 100 % zoom, light and dark. Captures are in
`media/` (`<kind>-light.webp`, `<kind>-dark.webp`; `before-*` where the face changed). To repeat:

    cargo build --release -p kabl-ui --bin kabl-ui
    sh docs/sound-engines/scripts/capture_faces.sh OUTDIR [DISPLAY]
    sh docs/sound-engines/scripts/capture_help.sh OUTDIR [DISPLAY]

Both need Xvfb, xdotool and a window manager (metacity here; openbox is not installed). The
scripts use `docs/rack-migration/drive.py`.

## What was wrong, and what changed

| face | before | after |
|---|---|---|
| `osc.fm6` | 54 controls in one row far wider than the screen; Algorithm and Ratio strips drew their labels over each other | 1230 px wide, fits a 1280 px screen. Header: jacks, Feedback, Index, Algorithm (dropdown). Below: six framed operator blocks, each with Ratio (dropdown), Fine, Level, Vel and Att / Dec / Sus / Rel. Base frequency, global fine and oversampling stay in the advanced area (`+3`) |
| `osc.fm` | Ratio strip: 33 labels overlapped | Ratio dropdown |
| `osc.wt` | Table strip overlapped; title cut by the `+3` button | Table dropdown; module 8 units wide |
| `quantizer` | Scale and Root strips overlapped | dropdowns |
| `random` | Length strip (0–16) overlapped | dropdown |
| `comparator`, `crossfade` | title under the `+1` button | module 6 units wide |
| `sample.hold`, `slew`, `logic`, `pan`, `attenuverter` | no fault found | unchanged |

## The selector fix

`crates/ui/src/routing.rs`, `stepped_selector`: when a segment is too narrow for the longest
option label (6.9 px per character at 11.5 px mono, scaled by zoom), the control is drawn as a
dropdown (`egui::ComboBox`) of the same options instead of segments. It applies to every module.
Opening the list inspects the control, as picking a segment does. A selector that fits is drawn
exactly as before. The per-option targets `sel:<id>.<param>.<k>` that scripts use exist only for
segmented selectors; a dropdown records `knob:<id>.<param>`.

## The FM voice face

Built-in faces are laid out by `place_local` in `crates/ui/src/rack.rs`, with kind-specific
branches (sequencer, MIDI In, ADSR); `ModuleSkin` is only for user art and is off by default, and
panel authoring is for composites. So the face is `fm6_face` there, plus a `Decor::Operators`
frame drawing, `width_units: 41` in the module, and short per-operator labels in `param_label`
("Att 3", "Rel 3"). A gap down the middle keeps the "Edit face" caption clear. The layout test
`controls_stay_inside_their_panel_and_apart` still runs on it; for this one module its knob boxes
are shrunk 8 px by 6 px because six envelopes in a row cannot keep the usual padding.

## Attenuverter

`amount` and `offset` keep their range (−24 to 24), default and stored values, so saved patches
sound the same. Their knobs now use a new `Taper::Cubic`: travel is cubic about the centre, so
the value changes slowly near 0 and quickly at the ends. A 1 % step of knob travel near unity
moves `amount` by about 0.17 (linear: 0.48). Modulation and host automation travel along the same
curve. Test: `attenuverter_knob_is_cubic_and_round_trips`.

## Help and Explain

For each module the Explain page and the Help panel were opened in the running editor
(`media/help-explain.webp`, `media/help-panel.webp`) and read: each shows the module's summary and
every param's help, with the new short labels. Not checked this way: the per-control Explain page
(its button lies below the visible part of the long fm6 list).
