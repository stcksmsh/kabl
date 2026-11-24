# Composite faces: rich panels

Design for letting a composite face show more than knobs, a plain selector and jacks: sliders,
toggles, icon selectors, live displays and buttons that act on the composite's insides. Owner
request, 2026-11-24: "The composites should have MANY different ways of appearing, they should
be able to have in/out jacks, knobs, sliders, buttons (like cues of their own or the
sine/triangle/... selection things), facets for visuals like the waveforms do etc."

This is a design, not code. Every reference is `file:line` on `origin/master` at `69660507`.

## 1. What exists today

### 1.1 The composite panel

- `Kind { Knob, Selector, Jack }` (`crates/core/src/panel.rs:10-14`); `Placement { kind, x, y,
  width, height }` with `deny_unknown_fields` (`panel.rs:15-24`); `Panel { width, placements,
  light, dark }` keyed by the exposure ID it binds (`panel.rs:34-40`).
- A placement must bind an exposure of the same composite: a jack binds `ports`, anything else
  binds `controls` (`panel.rs:93-101`). Minimum footprints per kind (`panel.rs:102-106`), face
  bounds with header and footer reserved (`panel.rs:107-116`), no overlap (`panel.rs:117-126`).
  Panel height is fixed at 340 (`panel.rs:6`), width 180-840 in 30-point units (`panel.rs:82-87`).
- The UI check adds the parameter rule: a selector binds a stepped param, a knob a continuous
  one, and a selector gets at least 18 points per option (`crates/ui/src/panels.rs:35-43`).
- Exposures alias a leaf target, never a second model: `Exposure { label, target, also }`
  (`crates/core/src/composite.rs:15-22`); a control must be a module parameter
  (`composite.rs:113-121`). Interface IDs come from `next_interface` (`composite.rs:56`).
- Drawing: `draw_authored_face` (`crates/ui/src/lib.rs:5728`) loops the placements
  (`lib.rs:5884`). Jacks are drawn inline (`lib.rs:5894-5961`); controls go through
  `panel_control_geo` (`lib.rs:5532-5542`) into `routing::param_knob` or
  `routing::stepped_selector` (`lib.rs:5989-6011`), the same functions module faces use.
  Reveal and focus find an authored control through `authored_control_geo` (`lib.rs:5506-5531`).
- A composite with no panel gets the default face, whose controls are
  `perform::param_editor` rows (egui slider or segments; `lib.rs:5585`, `perform.rs:477`,
  `perform.rs:519`).
- Authoring: the panel editor lists each exposure with Place/Unplace (`panels.rs:353-411`).
  Place picks Jack, Selector or Knob from the exposure and param taper (`panels.rs:372-392`) at a
  fixed spot and default size (`panels.rs:394-410`); the author then types x, y, w, h
  (`panels.rs:423-438`).

### 1.2 Inventory of module-face controls and displays

The brief expected sliders, toggles and icon selectors on module faces. They are not there:
a module face places only two per-parameter geometries, `Geo::Knob` and `Geo::Select`
(`crates/ui/src/rack/mod.rs:181-184`), and everything else is a module-level `Decor`
(`rack/mod.rs:228-245`). Sliders and toggles exist elsewhere in the app; they are listed so the
design can reuse them.

| Element | Scope | Drawn by | Data / effect | Notes |
|---|---|---|---|---|
| Knob | per param, continuous | `routing::param_knob` (`routing.rs:710`) → `kit::face::knob` (`face.rs:486`, called `routing.rs:906`), modulation span `face::mod_range` (`face.rs:639`, `routing.rs:978`), value `face::readout` (`face.rs:792`, `routing.rs:1105`) | `editor.set_param`; route lanes and plugs | Module call `lib.rs:4442-4461`; composite call `lib.rs:5990` |
| Stepped selector | per param, stepped | `routing::stepped_selector` (`routing.rs:1412`): text segments, or a dropdown when segments are too narrow (`routing.rs:1454-1482`) | `editor.set_param`; option names from `routing::step_labels` (`routing.rs:227`) | Waveforms are text: `SIN TRI SAW SQR` (`routing.rs:236`), `SIN TRI SAW SQR S&H` (`routing.rs:235`). No icon selector exists |
| Sequencer step cell | per param (seq gate/glide slots) | `seq_step_cell` (`lib.rs:3952`), chosen at `lib.rs:4433-4440` | on/off cell in the step row | Only `seq` |
| Jack | per port | `kit::face::jack` (`face.rs:692`), `face::jack_ch` (`face.rs:707`, used `lib.rs:4692`) | cable endpoints | Composite jacks are hand-drawn rings, `lib.rs:5907-5909` |
| Display | module | `Decor::Display` (`rack/mod.rs:234`), placed for 21 kinds (`rack/mod.rs:689-698`) and sparse fx faces (`rack/fx.rs:27,44,50`); drawn `lib.rs:3142-3162` → `displays::draw` (`displays.rs:375`), kind dispatch `displays.rs:397-403`, `fx_displays::draw` (`fx_displays.rs:149-166`), `drum_displays::draw` (`drum_displays.rs:28`) | Params (`displays.rs:365`) plus engine face taps: the face asks `wants.display` (`displays.rs:112`) and `wants.feeds` (`displays.rs:125`), `Live::frame` sends `Command::Faces` (`displays.rs:209`), at most 16 taps (`crates/engine/src/facetap.rs:17`), read back with `Live::tap` (`displays.rs:284`) | Styling: `face::display_frame` (`face.rs:845`), `face::trace` (`face.rs:937`), `face::playhead` (`face.rs:978`). Display code reads only `m.id` and `m.info` of `Placed` |
| Wavetable edit chip | module (`osc.wt`) | `lib.rs:3163-3180` | opens the table editor | |
| Transport buttons | module (`clock`) | `Decor::Transport` (`rack/mod.rs:236`), `draw_transport` (`lib.rs:3546`) | runtime command, `perform::fire` `"run"`/`"restart"` (`perform.rs:445-446`) | Refused under host clock (`perform.rs:420-423`) |
| Cue pads | module (`cues`) | `Decor::Cues` (`rack/mod.rs:242`), `draw_cues` (`lib.rs:4106`) | `cues::launch` (`cues.rs:349`); cues can move macros with a morph (`cues.rs:7-9`) | `fire` actions `cue.N`, `cancel` (`perform.rs:430-440`) |
| Bank buttons | module (`seq`) | `Decor::Banks` (`rack/mod.rs:238`), `draw_banks` (`lib.rs:3665`) | bank launch; `fire` action `bank.N` (`perform.rs:426`) | |
| Keys strip | module (`midi.in`) | `Decor::Keys` (`rack/mod.rs:232`, `lib.rs:3200`) | text summary | |
| FM operators | module (`osc.fm6`) | `Decor::Operators` (`rack/mod.rs:240`), `fm6::draw` (`lib.rs:3140`), trace `fm6.rs:370` | params | |
| Drum grid | module (`drum.grid`) | `Decor::Grid` (`rack/mod.rs:244`), `drum_grid::draw` (`lib.rs:3223`) | step editor | |
| Slider (not on faces) | per param | `kit::slider` (`kit/controls.rs:571`); Perform card `param_value` (`perform_view.rs:1693`, slider `perform_view.rs:1731`) | taper through `to_norm`/`from_norm`, `set_param_gesture` | |
| Toggle (not on faces) | per param / menu | painter toggle `kit::parts::toggle` (`kit/parts.rs:78`), widget `kit::toggle` (`kit/controls.rs:507`, used `perform.rs:651`) | | |
| Skin Switch / Readout | per param | declared, not rendered (`crates/modules/src/skin.rs:19-31`) | | |

Parameter metadata has no control-kind field: only `Taper { Linear, Exponential, Stepped, Cubic }`
(`crates/modules/src/info.rs:112-121`) and range (`info.rs:162-171`). The control kind is the
face's choice, which is why it can live on the composite placement.

## 2. Proposed element set

Per-parameter controls (bind a `controls` exposure, as today):

| Kind | Binds | Allowed taper | Drawn with |
|---|---|---|---|
| `knob` | control | continuous | `routing::param_knob` (as today) |
| `selector` | control | stepped | `routing::stepped_selector` (as today) |
| `slider` | control | continuous | new face slider, same value path as `perform_view::param_value` (`perform_view.rs:1729-1740`) |
| `toggle` | control | stepped, exactly two options | `kit::parts::toggle` (`kit/parts.rs:78`) + `editor.set_param` |
| `icons` | control | stepped, with a built-in glyph set for (module kind, param) | `stepped_selector` geometry with glyphs instead of text; glyphs from `displays::wave` (`displays.rs:436`), `displays::plot` (`displays.rs:405`), `face::trace` |

Per-port: `jack` (as today).

Composite-level elements (bind a leaf, not an exposure):

| Kind | Binds | Drawn with |
|---|---|---|
| `display` | a leaf module inside the composite whose kind has a display | `displays::draw` (`displays.rs:375`), tap request `wants.display` + `wants.feeds` (`displays.rs:112,125`) |
| `button` | a leaf module and one `perform::fire` action (`perform.rs:419`): `cue.N`, `cancel` on a `cues` leaf, `bank.N` on a `seq` leaf, `run`, `restart` on a `clock` leaf | `kit::parts::chip` style pad or the cue pad look of `draw_cues`; press calls `fire` |

Rule for a control kind the param cannot support: the panel is invalid and the editor never
offers it. The editor's kind chooser lists only the kinds allowed by the taper and option count
in the table above; `panels::validate` (`panels.rs:35`) extends its stepped/continuous check to
the same table and names the binding in the error, as it does today. An `icons` placement on a
param without a glyph set is invalid. Loading never rewrites a kind.

## 3. Storage

### 3.1 Schema diff against `crates/core/src/panel.rs`

```diff
 pub enum Kind {
     Knob,
     Selector,
     Jack,
+    Slider,
+    Toggle,
+    Icons,
+    Display,
+    Button,
 }

 pub struct Placement {
     pub kind: Kind,
     pub x: f32,
     pub y: f32,
     pub width: f32,
     pub height: f32,
+    /// Display and Button: the leaf module this element reads or acts on. Any leaf of the
+    /// composite, nested ones included. None for exposure-bound kinds.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub source: Option<ModuleId>,
+    /// Button: one `perform::fire` action for `source`. None otherwise.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub action: Option<String>,
+    /// Display and Button: printed name. Exposure-bound kinds keep using the exposure label.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub label: Option<String>,
 }
```

- Keys: exposure-bound kinds keep the exposure ID as key (`panel.rs:93-96`). `display` and
  `button` take a fresh key from `next_interface` (`composite.rs:56`), so they never collide with
  an exposure, and a placement key that is also an exposure ID must be an exposure-bound kind.
- Validation, `Panel::validate`: kind decides the binding (`controls` for knob, selector,
  slider, toggle, icons; `ports` for jack; `source` present and `source`, `action`, `label`
  absent elsewhere). Minimum sizes extend the table at `panel.rs:102-106` (slider 120x55,
  toggle 64x55, icons 18 per option x 55, display 120x60, button 54x40; exact numbers are set
  by the drawing slice's captures). `label` at most 40 characters; `action` must parse as one of
  the five actions. Footprint and overlap rules are unchanged.
- Validation needing the patch (`panels::validate`, `panels.rs:19`): `source` is in
  `composite::leaves(state, id)` (`composite.rs:105`), a display source's kind has a display
  (the dispatch lists at `displays.rs:397-403`, `fx_displays.rs:149-166`), and a button's
  action matches its source kind.
- Lifetime: `Op::RemoveModule` today keeps only placements bound to a live exposure
  (`crates/core/src/state.rs:326-328`). It must also keep a `source` placement whose module
  still exists, and drop one whose source was removed. Insert and duplicate remap leaf IDs
  through `remap` (`crates/ui/src/composites.rs:634`); `source` goes through the same ID map.

### 3.2 Compatibility

- Old files load unchanged: every new field defaults to `None`, and `knob`, `selector`, `jack`
  keep their spelling. No existing patch changes sound or look.
- `Placement` and `Panel` deny unknown fields, so a v9 build cannot read a new kind. Patch
  files follow the rule already used for cable params and fan-in (`format.rs:23-48`): add v10,
  and `schema_for` writes v10 only when a composite in the log has a placement of a new kind,
  else the lowest version that fits (v8 or v9). `check_schema` accepts 8..=10
  (`format.rs:127`). A v9 build then says "newer schema" instead of a serde error.
- `.kablc` / library packages: same rule at `composites.rs:547-555`: schema 3 only when a new
  kind is present, else 1 or 2; `decode` accepts 1..=3 (`composites.rs:572`).
- All new variants and fields land in one schema step (decision D4), so v10 means one shape.

## 4. Drawing

`draw_authored_face`'s placement loop (`lib.rs:5884`) becomes a `match p.kind`:

- `jack`: unchanged (`lib.rs:5894-5961`).
- `knob`, `selector`: unchanged (`lib.rs:5989-6011`).
- `slider`: a face slider in the footprint's control rect. Value path copied from
  `perform_view::param_value` (`perform_view.rs:1729-1740`: `to_norm`, `from_norm`,
  `set_param_gesture`), the route badge from `routing::selector_plug` (`routing.rs:1524`), the
  label from `face_label` (`lib.rs:6042`). Look: decision D2.
- `toggle`: `kit::parts::toggle` (`kit/parts.rs:78`) painted in the control rect; a click sets
  the other option with `editor.set_param`, inspect as `stepped_selector` does
  (`routing.rs:1513-1517`).
- `icons`: `stepped_selector` (`routing.rs:1412`) gains a glyph mode: same segments, hit IDs
  (`sel:{key}.{k}`) and plug; each segment draws `face::trace` of `displays::plot` over
  `displays::wave` (`displays.rs:436`) for sine, triangle, saw, square, and the LFO's
  sample-and-hold steps (`displays.rs:727-740`). Glyph sets: `osc.va` and `lfo` `waveform`
  (decision D3).
- `display`: `displays::draw` takes `(id, info)` instead of `&Placed`, since it reads nothing
  else (all 29 uses are `m.id` and `m.info` in `displays.rs`, `fx_displays.rs`,
  `drum_displays.rs`). The face calls `wants.display` and `wants.feeds` for the source when the
  footprint is on screen, as module faces do (`lib.rs:3146-3150`), then `displays::draw` in the
  footprint. A source inside a nested composite works the same: taps are by module ID, and the
  same module shown on its own face and on the composite face asks once (`displays.rs:176-182`).
  The 16-tap budget (`facetap.rs:17`) is shared; past it the display shows its parameter-only
  picture, as module faces already do.
- `button`: a pad; press calls `perform::fire(editor, view, source, action)` (`perform.rs:419`),
  so host-clock refusal and cue timing are inherited. The lit state reuses `cues::standing_now`
  (`cues.rs:408`) for cue buttons and the bank state for bank buttons.
- `panel_control_geo` (`lib.rs:5532`) maps slider, toggle and icons to `Geo::Select { rect }`
  so reveal, focus and pulse rings (`lib.rs:6014-6030`) work unchanged.

Plugin build: the CLAP editor draws the same rack through `kabl_ui::show`
(`crates/clap/src/lib.rs:795`), so every element appears there with no extra work. A button's
`run`/`restart` under the host clock shows the existing refusal (`perform.rs:420-423`).

Perform page: pins are `(module, param or special key)` (`perform.rs:44-49`) and are offered
only from a module's right-click menu (`perform.rs:642`, `lib.rs:2957`). Composite controls
alias leaf params, so a pin on the leaf already covers them; the composite face has no pin entry.
Whether to add one is decision D6.

## 5. Authoring

In the panel editor (`panels::editor`, `panels.rs:144`):

1. Each control exposure row gets a kind chooser next to Place, listing only the kinds its
   param allows (section 2). Place uses the chosen kind and its default size
   (`panels.rs:394-410` table extended). The default stays knob or selector as today.
2. A new "Add display" row lists the composite's leaves that have a display (nested leaves
   included, shown as `Composite / Module`); choosing one adds a `display` placement with a
   fresh key and the module name as label.
3. A new "Add button" row lists actions of the composite's `cues`, `seq` and `clock` leaves
   (`Cue 3 · <cue name>`, `Bank B`, `Run`, `Restart`); choosing one adds a `button` placement.
4. Position and size stay the numeric x, y, w, h fields (`panels.rs:423-438`). Applying an
   invalid panel shows the `Panel::validate` message (overlap, too small, outside the face) and
   is refused, as today. Changing the panel width so an element falls outside refuses the same
   way. Dragging footprints on a preview is decision D8.

## 6. Slice plan

Each slice branches from `origin/master`, keeps existing patches sounding and looking the same,
and is checked by `cargo +1.99.0 clippy --workspace --all-targets -- -D warnings` and
`cargo +1.99.0 test --workspace`. Captures are light and dark, of a composite face, under the
job scratch dir.

1. **Stored form (v10).**
   Scope: `crates/core/src/panel.rs`, `crates/core/src/format.rs`, `crates/core/src/state.rs`,
   `crates/ui/src/composites.rs`, `crates/ui/src/panels.rs`, tests.
   Done means: (a) the section 3.1 diff with serde defaults; (b) `Panel::validate` and
   `panels::validate` accept knob, selector, jack exactly as before and reject every new kind
   with "not supported yet" (later slices lift this per kind); (c) a v9 fixture and a v8 fixture
   load and re-save byte-identical; (d) `schema_for` and package schema tests: a new-kind panel
   writes v10 / package 3, a v9-shaped one does not; (e) `RemoveModule` keeps a `source`
   placement whose module lives and drops one whose module is removed; (f) insert and
   duplicate remap `source`.
2. **Slider.**
   Scope: `crates/ui/src/lib.rs` (authored face), `crates/ui/src/panels.rs`, tests.
   Done means: slider accepted on continuous params and refused on stepped ones (test);
   kind chooser offers it; dragging it changes the leaf param with taper and one undo entry per
   gesture (test through recorded hit rects); route badge shows; captures.
3. **Toggle.**
   Scope: as slice 2. Done means: toggle accepted only on two-option stepped params (test,
   e.g. `vca.exponential`, `arp.latch`); click flips the value, undoable; captures.
4. **Icon selector.**
   Scope: `crates/ui/src/routing.rs`, `crates/ui/src/displays.rs` (glyph helper),
   `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, tests. Done means: `icons` accepted only
   on `osc.va.waveform` and `lfo.waveform` (test); each segment's hit rect selects its option
   (test); glyphs drawn from the same functions the displays use; captures at zoom 1 and the
   smallest face zoom.
5. **Display.**
   Scope: `crates/ui/src/displays.rs`, `fx_displays.rs`, `drum_displays.rs` (signature),
   `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, tests. Done means: "Add display" lists
   leaves with displays, nested included (test); a display placement on a closed composite asks
   the engine for the leaf's tap (test on `wants` output); a leaf in a nested composite draws;
   module-face displays unchanged (existing display tests pass); captures with audio running
   (offline render of the frame, no sound on the default output).
6. **Button.**
   Scope: `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, `crates/ui/src/perform.rs` (make
   `fire` reachable if needed), tests. Done means: "Add button" lists the cue, bank and clock
   actions of the composite's leaves (test); pressing a cue button launches that cue on the
   inside `cues` module (test on queued launch); host-clock refusal holds (test); lit state
   follows the cue/bank state; captures.
7. **Drag placement** (only if D8 = yes). Scope: `crates/ui/src/panels.rs`, tests. Done means:
   footprints drag and resize on a preview with 6-point snap; an overlapping or out-of-bounds
   drop is shown red and not applied (test); captures.
8. **Pin composite controls to Perform** (only if D6 = yes). Scope: `crates/ui/src/lib.rs`,
   `crates/ui/src/perform.rs`, tests. Done means: right-click on a composite face control offers
   "Pin to Perform", which pins the leaf param under the exposure label (test); captures.

## 7. Owner decisions

**D1. What "cues of their own" means.**
- A. A button fires an existing action on a leaf inside the composite: a `cues` module inside
  it launches its cues (which can move inside macros with a morph, `cues.rs:7-9`), a `seq`
  inside it switches banks, a `clock` inside it runs or restarts. No new runtime, no new stored
  state beyond the button; reuses `perform::fire` and the cue engine.
- B. Snapshot buttons: the composite stores N named sets of its exposed control values; a
  button jumps or morphs all of them. New stored data per composite, and a morph of arbitrary
  params, while the engine morph only moves macros (`kabl_engine::morph`,
  `MAX_MACRO_TARGETS = 16`).
- C. Momentary trigger: a button sends a gate into an inside input (a manual hit). Needs a new
  engine command or a new module; no such path exists today.
- Recommendation: **A**. It gives a composite its own cues by putting a `cues` module inside it,
  needs no new model, and B or C can follow as their own designs if A is not enough.

**D2. Slider look.**
- A. Horizontal slider using the app's existing slider look (`kit::slider`).
- B. Vertical fader drawn in the face style (new drawing, mixer-like).
- Recommendation: **A** first; judge on the slice 2 captures, B only if A reads wrong on a face.

**D3. Icon sets.**
- A. Built-in glyphs only for `osc.va` and `lfo` waveform, drawn from the display code.
- B. Also let an author supply a PNG per option in the package (like panel art).
- Recommendation: **A**. B adds a format field and image validation for a case nobody has asked
  for yet.

**D4. Schema rollout.**
- A. All new kinds and fields in one v10 step (slice 1); later slices only lift validation.
- B. One schema bump per kind as each ships.
- Recommendation: **A**. One format change to review, and dev builds between slices never
  meet a file shape they cannot parse.

**D5. Display and button sources in nested composites.**
- A. Any leaf of the composite, nested included (`composite::leaves`).
- B. Direct members only.
- Recommendation: **A**. Taps are by module ID, so nesting costs nothing, and B would force
  authors to flatten.

**D6. Composite controls on the Perform page.**
- A. No change: pin the leaf from inside the composite, as today.
- B. Add "Pin to Perform" on composite face controls (slice 8).
- Recommendation: **A** for this batch; B is cheap later if wanted.

**D7. Whole module blocks on a face** (the full cue pad grid, transport block, bank strip).
- A. Single buttons only; an author places as many as wanted.
- B. Also offer each block as one element.
- Recommendation: **A**. Single buttons cover the same actions with free layout.

**D8. Placement by dragging.**
- A. Keep numeric x, y, w, h fields.
- B. Add drag and resize on a preview (slice 7).
- Recommendation: **B**. With five more kinds, typing coordinates becomes the slow part of
  authoring a face.
