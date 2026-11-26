# Composite faces: rich panels

Design for letting a composite face show more than knobs, a plain selector and jacks: sliders,
toggles, icon selectors, live displays, buttons that act on the composite's insides, snapshots,
and whole module blocks, all laid out on a dedicated design page. Owner request, 2026-11-24:
"The composites should have MANY different ways of appearing, they should be able to have
in/out jacks, knobs, sliders, buttons (like cues of their own or the sine/triangle/...
selection things), facets for visuals like the waveforms do etc."

This is a design, not code. Every reference is `file:line` on `origin/master` at `266bb279`.
The owner's decisions (section 8) were given on 2026-11-24 and the sections before them follow
them.

## 1. What exists today

### 1.1 The composite panel

- `Kind { Knob, Selector, Jack }` (`crates/core/src/panel.rs:10-14`); `Placement { kind, x, y,
  width, height }` with `deny_unknown_fields` (`panel.rs:15-24`); `Artwork { name, png }`
  (`panel.rs:27-31`); `Panel { width, placements, light, dark }` keyed by the exposure ID it
  binds (`panel.rs:34-40`).
- `Artwork::validate` (`panel.rs:51-79`): a plain local name (no paths), PNG magic and IHDR,
  at most `MAX_IMAGE_BYTES` = 128 KiB (`panel.rs:7`), at most 2048 per edge and 1 megapixel.
  The document caps all artwork at 8 megapixels (`crates/core/src/composite.rs:221-234`); the UI
  decodes it once more (`crates/ui/src/panels.rs:13-18`).
- A placement must bind an exposure of the same composite: a jack binds `ports`, anything else
  binds `controls` (`panel.rs:93-101`). Minimum footprints per kind (`panel.rs:102-106`), face
  bounds with header and footer reserved (`panel.rs:107-116`), no overlap (`panel.rs:117-126`).
  Panel height is fixed at 340 (`panel.rs:6`), width 180-840 in 30-point units (`panel.rs:82-87`).
- The UI check adds the parameter rule: a selector binds a stepped param, a knob a continuous
  one, and a selector gets at least 18 points per option (`crates/ui/src/panels.rs:35-43`).
- Exposures alias a leaf target, never a second model: `Exposure { label, target, also }`
  (`composite.rs:15-22`); a control must be a module parameter (`composite.rs:236-241`).
  Interface IDs come from `next_interface` (`composite.rs:56`). A composite has at most 32
  controls and 32 ports (`composite.rs:163-164`), nests at most 8 deep and cannot own itself
  (`composite.rs:180-191`).
- Drawing: `draw_authored_face` (`crates/ui/src/lib.rs:5728`) loops the placements
  (`lib.rs:5884`). Jacks are drawn inline (`lib.rs:5894-5953`); controls go through
  `panel_control_geo` (`lib.rs:5532-5542`) into `routing::param_knob_named` or
  `routing::stepped_selector` (`lib.rs:5981-6004`), the same functions module faces use.
  Reveal and focus find an authored control through `authored_control_geo` (`lib.rs:5506-5531`).
  The face has no right-click menu.
- A composite with no panel gets the default face, whose controls are
  `perform::param_editor` rows (egui slider or segments; `lib.rs:5585`, `perform.rs:477`,
  `perform.rs:519`).
- Authoring: a tool window (`panels.rs:179`) opened by "Edit face" from the default face
  (`lib.rs:5609-5613`), the authored face (`lib.rs:5843-5851`) and the composite list
  (`crates/ui/src/composites.rs:1180`). It lists each exposure with Place/Unplace
  (`panels.rs:353-411`); Place picks Jack, Selector or Knob from the exposure and param taper
  (`panels.rs:372-392`) at a fixed spot and default size (`panels.rs:394-410`); the author then
  types x, y, w, h (`panels.rs:423-438`). Edits go to a draft (`panels::View`,
  `panels.rs:99-107`); Apply writes it with `composites::set` as one project undo step
  (`panels.rs:454`, `panels.rs:484-500`); Reload throws the draft away (`panels.rs:501-506`).
- `Op::SetComposite` carries the whole composite, artwork included (`crates/core/src/op.rs:55-58`).

### 1.2 Inventory of module-face controls and displays

A module face places only two per-parameter geometries, `Geo::Knob` and `Geo::Select`
(`crates/ui/src/rack/mod.rs:181-184`), and everything else is a module-level `Decor`
(`rack/mod.rs:228-245`). Sliders and toggles exist elsewhere in the app; they are listed so the
design can reuse them.

| Element | Scope | Drawn by | Data / effect | Notes |
|---|---|---|---|---|
| Knob | per param, continuous | `routing::param_knob` (`routing.rs:710`) → `kit::face::knob` (`face.rs:486`, called `routing.rs:925`), modulation span `face::mod_range` (`face.rs:639`, `routing.rs:997`), value `face::readout` (`face.rs:792`, `routing.rs:1124`) | `editor.set_param`; route lanes and plugs | Module call `lib.rs:4442-4461`; composite call `routing::param_knob_named` (`routing.rs:728`, `lib.rs:5982`) |
| Stepped selector | per param, stepped | `routing::stepped_selector` (`routing.rs:1431`): text segments, or a dropdown when segments are too narrow (`routing.rs:1473-1501`) | `editor.set_param`; option names from `routing::step_labels` (`routing.rs:227`) | Waveforms are text: `SIN TRI SAW SQR` (`routing.rs:236`), `SIN TRI SAW SQR S&H` (`routing.rs:235`). No icon selector exists |
| Sequencer step cell | per param (seq gate/glide slots) | `seq_step_cell` (`lib.rs:3952`), chosen at `lib.rs:4433-4440` | on/off cell in the step row | Only `seq` |
| Jack | per port | `kit::face::jack` (`face.rs:692`), `face::jack_ch` (`face.rs:707`, used `lib.rs:4692`) | cable endpoints | Composite jacks are hand-drawn rings, `lib.rs:5907-5909` |
| Display | module | `Decor::Display` (`rack/mod.rs:234`), placed for 21 kinds (`rack/mod.rs:689-698`) and sparse fx faces (`rack/fx.rs:27,44,50`); drawn `lib.rs:3142-3162` → `displays::draw` (`displays.rs:375`), kind dispatch `displays.rs:397-403`, `fx_displays::draw` (`fx_displays.rs:149-166`), `drum_displays::draw` (`drum_displays.rs:28`) | Params (`displays.rs:365`) plus engine face taps: the face asks `wants.display` (`displays.rs:112`) and `wants.feeds` (`displays.rs:125`), `Live::frame` sends `Command::Faces` (`displays.rs:209`), at most 16 taps (`crates/engine/src/facetap.rs:17`), read back with `Live::tap` (`displays.rs:284`) | Styling: `face::display_frame` (`face.rs:845`), `face::trace` (`face.rs:937`), `face::playhead` (`face.rs:978`). Display code reads only `m.id` and `m.info` of `Placed` (checked in section 5.6) |
| Wavetable edit chip | module (`osc.wt`) | `lib.rs:3163-3180` | opens the table editor | |
| Transport buttons | module (`clock`) | `Decor::Transport` (`rack/mod.rs:236`), `draw_transport(.., id, r)` (`lib.rs:3546-3554`) | runtime command, `perform::fire` `"run"`/`"restart"` (`perform.rs:445-446`) | Refused under host clock (`perform.rs:420-423`). Hit key `"{key}:{id}"` and egui ID `("kabl-transport", key, id)` (`lib.rs:3584-3587`) |
| Cue pads | module (`cues`) | `Decor::Cues` (`rack/mod.rs:242`), `draw_cues(.., id, r, now)` (`lib.rs:4106-4115`) | `cues::launch` (`cues.rs:349`); cues can move macros with a morph (`cues.rs:7-9`) | `fire` actions `cue.N`, `cancel` (`perform.rs:430-440`); hit key `cue:{id}.{n}` (`lib.rs:4146`) |
| Bank buttons | module (`seq`) | `Decor::Banks` (`rack/mod.rs:238`), `draw_banks(.., id, r, now, drawn)` (`lib.rs:3665-3675`) | bank launch; `fire` action `bank.N` (`perform.rs:426`) | |
| Keys strip | module (`midi.in`) | `Decor::Keys` (`rack/mod.rs:232`, `lib.rs:3200`) | text summary | |
| FM operators | module (`osc.fm6`) | `Decor::Operators` (`rack/mod.rs:240`), `fm6::draw` (`lib.rs:3140`), trace `fm6.rs:370` | params | |
| Drum grid | module (`drum.grid`) | `Decor::Grid` (`rack/mod.rs:244`), `drum_grid::draw` (`lib.rs:3223`) | step editor | |
| Slider (not on faces) | per param | `kit::slider` (`kit/controls.rs:571`); Perform card `param_value` (`perform_view.rs:1693`, slider `perform_view.rs:1731`) | taper through `to_norm`/`from_norm`, `set_param_gesture` | |
| Toggle (not on faces) | per param / menu | painter toggle `kit::parts::toggle` (`kit/parts.rs:78`), widget `kit::toggle` (`kit/controls.rs:507`, used `perform.rs:651`) | | |
| Skin Switch / Readout | per param | declared, not rendered (`crates/modules/src/skin.rs:19-31`) | | |

Parameter metadata has no control-kind field: only `Taper { Linear, Exponential, Stepped, Cubic }`
(`crates/modules/src/info.rs:112-121`) and range (`info.rs:161-171`). The control kind is the
face's choice, which is why it can live on the composite placement. `ParamInfo::to_norm` and
`from_norm` (`info.rs:126-158`) map a value to knob travel and back; `from_norm` rounds a stepped
param to the nearest option.

### 1.3 The cue morph engine

- `kabl_engine::morph` moves macros only: a target is `MacroTarget { id, index, value }`
  (`crates/engine/src/morph.rs:28-34`) written as `RuntimeTarget::Param { kind: "macro" }`
  (`morph.rs:63-69`). A launch carries at most `MAX_MACRO_TARGETS = 16` (`morph.rs:24`); the
  engine drives at most `MAX_DRIVEN = 16` at once (`morph.rs:26`) in fixed arrays
  (`morph.rs:72-75`). A full table drops the new morph (`morph.rs:82-93`, `morph.rs:146-148`).
- Progress is the reference clock's own pulses; length 0 or a stopped clock jumps; the next
  morph starts from where the value is; an outside write ends a morph; the engine holds the
  target afterwards (`morph.rs:1-16`). Interpolation is linear in the stored value
  (`morph.rs:190-191`). Nothing allocates.
- `drive_morphs` writes the values into every graph with `drive_runtime` (set now, no ramp;
  `crates/engine/src/patch_engine.rs:897-917`, `crates/engine/src/compile.rs:2941-2945`). A value
  write (`ParamSet`) or new host automation releases the morph of that target
  (`patch_engine.rs:1227-1230`, `patch_engine.rs:1259-1263`).
- The UI sends values and commands through one ordered queue: "an action goes after every
  value requested before it" (`crates/ui/src/control.rs:363-392`), and the audio thread drains
  it in order (`patch_engine.rs:1265-1300`).
- Morph progress is reported per driven macro (`patch_engine.rs:891-895`) into a 64-entry ring
  (`crates/ui/src/app/audio.rs:23`, `audio.rs:260`, `crates/ui/src/app/callback.rs:190-192`;
  plugin `crates/clap/src/lib.rs:1190`), shown on Perform's macro card
  (`crates/ui/src/perform_view.rs:1458-1465`).

### 1.4 Perform pins

Pins are `(module, param or block key)` (`perform.rs:43-49`), stored as `pin.<key>`
presentation params on the leaf (`perform.rs:21`, `perform.rs:85-105`). Three block keys exist:
`TRANSPORT`, `BANKS`, `CUE_PADS` (`perform.rs:31-35`). Pins are offered only from a module's
right-click menu (`perform.rs:631-660`, `lib.rs:2957`). `pin_source` prints `Module #id ·
control` (`perform.rs:568-580`); `rename_pin` sets the shown name (`perform.rs:583-590`).

## 2. Element set

Per-parameter controls (bind a `controls` exposure, as today):

| Kind | Binds | Allowed taper | Drawn with |
|---|---|---|---|
| `knob` | control | continuous | `routing::param_knob_named` (as today) |
| `selector` | control | stepped | `routing::stepped_selector` (as today) |
| `slider` | control | continuous | new face slider, horizontal or vertical (section 5.2), value path of `perform_view::param_value` (`perform_view.rs:1729-1740`) |
| `toggle` | control | stepped, exactly two options | `kit::parts::toggle` (`kit/parts.rs:78`) + `editor.set_param` |
| `icons` | control | stepped | `stepped_selector` geometry with an icon per option: a built-in glyph set, or one author PNG per option |

Per-port: `jack` (as today).

Composite-level elements (no exposure; they carry a `source`):

| Kind | Source | Drawn with |
|---|---|---|
| `display` | a leaf module at any depth whose kind has a display | `displays::draw` (`displays.rs:375`), tap request `wants.display` + `wants.feeds` (`displays.rs:112,125`) |
| `button` | one of: a leaf module at any depth and one `perform::fire` action (`perform.rs:419`; `cue.N`, `cancel` on `cues`, `bank.N` on `seq`, `run`, `restart` on `clock`); a `button` on a composite nested at any depth; one of this composite's snapshots | a pad in the cue-pad look of `draw_cues` |
| `cue_pads` | a `cues` leaf at any depth | `draw_cues` (`lib.rs:4106`) |
| `banks` | a `seq` leaf at any depth | `draw_banks` (`lib.rs:3665`) |
| `transport` | a `clock` leaf at any depth | `draw_transport` (`lib.rs:3546`) |
| `snapshots` | this composite's snapshots | new strip: one pad per snapshot, lit on the last recalled |

Rule for a kind the binding cannot support: the panel is invalid and the design page never
offers it. The page's kind chooser lists only the kinds the taper and option count allow;
`panels::validate` (`panels.rs:19`) extends its stepped/continuous check (`panels.rs:35-43`) to
the same table and names the binding in the error, as it does today. Loading never rewrites a
kind.

## 3. Snapshots

A snapshot is a named set of values for the composite's exposed controls (decision D1).

### 3.1 Capture and contents

- Capture stores the current document value of every exposed control, keyed by the control's
  exposure ID, in the param's own units. A composite has at most 32 controls
  (`composite.rs:164`), so a snapshot holds at most 32 values.
- A control exposed after the capture is absent from it; on recall an absent control is left
  where it is (the rule cues use for macros, `cues.rs:7-9`). Re-capture replaces the values.
- Each snapshot has a stable `id`, taken from `next_interface` (`composite.rs:56`) like every
  other interface key and never reused, so deleting one snapshot never shifts what a button
  points at. Order in the list is display order only.
- Each snapshot has a name (at most 40 characters), a morph length in beats (0 = jump, at most
  `cues::MAX_MORPH_BEATS` = 16, `cues.rs:36`) and a reference clock (any `clock` module in the
  patch, like a cue's, `cues.rs:1-4`). At most 16 snapshots per composite.
- A clock outside the composite does not travel with a package: package export drops it (the
  snapshot then jumps until the author picks a clock after insert), as export already drops what
  crosses the boundary (`composites.rs:536-540`).

### 3.2 Recall

Values reach the audio thread only through `Delivery::sync` (`crates/ui/src/control.rs:161-199`):
it diffs the document against what was last sent, holds one `ParamSet` per changed target, and
flushes in the same call; when a compile is in flight or waiting, or the held values would pass
`MAX_HELD`, it compiles a graph instead (`control.rs:164-170`, `control.rs:188-195`), and a held
graph is pushed before any action (`control.rs:349-361`). Recall is built around that.

Recall of snapshot `s` on composite `C` happens in one frame, in two places on the UI thread:

1. **Where the button is pressed** (face drawing, which has the editor and `UiState` but not the
   `Delivery`). Compute, per control in `s`, the target value through the current target's
   `from_norm(to_norm(v))` (`info.rs:126-158`), so it is clamped to the param's range and a
   stepped value is a whole option, and keep only the controls whose document value changes.
   For each, the start value is the morph's live value when the UI's morph report shows one
   driving that param, else the current document value. Apply one undoable edit, "Snapshot
   <name>", that sets those controls with `SetParam` in one `Op::Group`; after it the document
   holds the target values, so a recompile, a save or an undo sees what the engine ends on. When
   the morph length is above 0 and a clock is set, also push a `MorphRequest { parts, targets }`
   onto a new `ui_state.morphs` list, the way launches are queued on `ui_state.launches`.
2. **In `control::deliver`** (`control.rs:455`, called once per frame by the app,
   `app/frame.rs:137`, and the plugin, `crates/clap/src/lib.rs:290`), which has
   `&mut Delivery`. Before its `d.sync(..)` it takes `ui_state.morphs` and keeps a request only
   when `held_actions` has room for all of its parts (`MAX_HELD_ACTIONS`, `control.rs:39`);
   nothing else queues actions between that check and step 2c, so the parts cannot be refused.
   Then:
   1. `sync` gets the kept requests' targets as a skip list (a new argument, a slice; no state
      is stored in `Delivery`). On the values path it holds no `ParamSet` for a skipped target;
      `sent` still records the document, so the next diff does not send it either.
   2. If `sync` took the compile path instead (a Load, a compile in flight or waiting, or the
      `MAX_HELD` fallback, `control.rs:164-170`, `control.rs:188-195`), the skip list was never
      used and the requests are dropped: the new graph is compiled from the document and plays
      the targets at once, a jump.
   3. On the values path the parts of each kept request are queued with `d.command`, after
      `sync`, so they follow every value `sync` held (`control.rs:363-392`).

A refused request (no room) is not kept, so its targets were not skipped and `sync` sends their
values: a jump. The skip list lives for one `sync` call, so it cannot outlive its recall.

**Why this is atomic.** For a morphing recall the only messages that reach the audio thread are
the morph parts: no `ToAudio::Set` for those targets is pushed, so no block can render at the
target before the morph starts, and no value can release the morph it belongs to. The audio
thread stages parts and starts every target of the recall in the same block, when the last part
arrives (section 3.3); until then the targets keep the values they had, which are the start
values (or the morph already moving them). A ring that is full delays the whole recall, never
half of it. A graph compiled later, for an unrelated edit while the morph runs, is built from
the document, which holds the targets; the engine writes its morph values into every graph each
block before rendering (`patch_engine.rs:897-917`, called at `patch_engine.rs:957`), so the swap
does not jump the morph.

A clock that is stopped when the morph lands: the engine jumps (existing rule, `morph.rs:8`).

Undo of a recall is one project undo step: its values release the morphs and put the old values
back. A hand on a control during a morph ends that control's morph (existing rule, `morph.rs:10`).

### 3.3 Engine change: morphs of any parameter

Changes in `crates/engine/src/morph.rs` and `patch_engine.rs`:

- **Target type.** `MacroTarget` stays the wire type inside `Launch` (`patch_engine.rs:129-138`),
  so cues and their tests do not change. Internally `Pending` and `Driven` hold
  `target: RuntimeTarget` instead of `(id, index)` (`morph.rs:36-61`) plus `from: Option<f32>`
  and `stepped: bool`; a launch converts each `MacroTarget` with today's `target()` mapping when
  it schedules (`patch_engine.rs:741-750`), and `target()` (`morph.rs:63-69`) is used only there.
- **Keys.** `schedule` matches a pending entry by `target` (`morph.rs:82-93`); `step` finds a
  driven entry by `target` (`morph.rs:136-139`) and writes `d.target` instead of rebuilding it
  (`morph.rs:200`).
- **Release.** `Morphs::release` (`morph.rs:100-115`) drops the driven entry whose `target`
  equals the written one, for any `RuntimeTarget::Param`, not only `kind: "macro"`. Its callers
  stay: a value write (`patch_engine.rs:1259-1263`) and a new host value
  (`patch_engine.rs:1227-1230`).
- **Taper.** The message carries no `ParamInfo`: the engine looks it up at start from the graph,
  as `set_runtime_with` already reads the module's info (`compile.rs:2947-2990`), through a new
  `CompiledPatch::runtime_info(target) -> Option<&'static ParamInfo>` next to `runtime_value`.
  Continuous params move in knob travel, `p.from_norm(lerp(p.to_norm(from), p.to_norm(to), x))`,
  so an exponential cutoff sweeps evenly by ear. A stepped param holds `from` until `x >= 0.5`
  and then jumps to `to`; it never passes through the options in between (a waveform going SIN
  to SQR never plays TRI or SAW). Macros from cues keep linear motion in their own units, as now
  (`morph.rs:190-191`).
- **Zipper.** Today the morph writes once per block with `drive_runtime`, set now with no ramp
  (`compile.rs:2941-2945`), and is smooth only because the macro module glides its value over
  20 ms (`morph.rs:15-16`). `drive_runtime` gains a `ramp` argument passed through to
  `set_runtime_with`: snapshot morphs pass `ramp = true` while the graph plays, so a continuous
  param the module does not smooth itself moves to each block's value over `RAMP_MS` in knob
  travel, starting from where it is (`compile.rs:2930-2936`); a param the module smooths, a
  stepped param and a cue's macro (`ramp = false`, as now) are set at once. The automation
  overlay is kept in both cases, as today.
- **Command and its size.** `Command` is unboxed and sits in `ToAudio` (`runtime.rs:54-61`) in
  a 256-slot ring (`control.rs:33`), in the app and in every plugin instance, and every
  `ToAudio::Set` is moved through a slot of that size, so the variant must not be the largest.
  The recall is sent as parts: `Command::Morph(MorphPart { seq: u32, last: bool, clock,
  pulses, count: u8, targets: [MorphTarget; 8] })`, with `MorphTarget { target: RuntimeTarget,
  from: f32, to: f32 }` (40 bytes: `RuntimeTarget` is 32). A part is about 350 bytes, under
  `Command::Launch` (`patch_engine.rs:129-138`, about 560 bytes, the largest variant today), so
  `size_of::<Command>()` and the ring do not grow; slice 8 pins that with a test. Cost: a
  32-control recall is 4 ring slots of 256. The audio thread copies parts into a fixed staging
  table of 32 (`MAX_MORPH_TARGETS`, one per exposed control, `composite.rs:164`) and schedules
  them all when the part with `last` arrives. No `Box`: dropping one on the audio thread would
  free memory there.
- **Limits.** `MAX_DRIVEN` rises from 16 to 64 (`morph.rs:26`), so two composites' recalls and
  a cue's 16 macros can move at once; pending and driven stay fixed arrays (`morph.rs:71-75`),
  and the per-block scan of 64 slots replaces one of 16.
- **A recall morph always lands.** For a recall the document already holds `to` and the UI sent
  no value for it (section 3.2), so a recall morph the engine abandons would leave the sound at
  `from` while the document says `to`. Every path that drops one therefore goes through one
  helper, `Morphs::land(target, to)`, which puts the final value into a land table that
  `drive_morphs` writes with `drive_runtime` in the same block and then empties. The table is
  keyed by target (landing a target already in it overwrites the entry) and is a fixed array of
  160 = 64 pending + 64 driven + 32 staged, so it holds every recall morph even when one reset
  abandons all of them at once. `Morphs::clear` (`morph.rs:78-80`) lands each recall morph and
  staged part it drops and does not touch the land table, so a land survives the clear that
  caused it. The paths: a pending morph
  whose clock is gone (`morph.rs:126-129`) or whose target does not resolve
  (`morph.rs:140-145`; the write is then a no-op), a full driven table (`morph.rs:146-148`) or
  pending table (`morph.rs:90-92`), an unfinished part set replaced by a new `seq`, host reset
  (`patch_engine.rs:218`) and cancel (`patch_engine.rs:817`). A Load (`patch_engine.rs:559`)
  needs none: its graph is compiled from the document, which holds `to`; the land its `clear`
  makes anyway writes `to` again, or nothing when the target is gone. Cue macros
  (`from: None`) are not landed, which keeps today's cue behaviour.
- **Report.** The progress report becomes `(RuntimeTarget, value, to, progress)`; the UI ring
  grows from 64 to 128 entries (`audio.rs:260`) and Perform's macro card keeps reading macros
  from it (`perform_view.rs:1458-1465`). Composite face controls show the live value while a
  morph runs, as the macro card does.

### 3.4 Edges

- **Control later un-exposed.** `remove_exposure` (`composites.rs:455-490`) already drops the
  placement (`composites.rs:486-488`); it also drops that key from every snapshot, in the same
  `SetComposite`. `composite::validate` refuses a snapshot key that is not a
  control, so a hand-edited file cannot carry a stale one.
- **Control retargeted.** No path retargets an exposure in place today: the only target rewrite
  is the ID remap on insert and duplicate (`composites.rs:731`), which keeps the same param. A
  future retarget path must drop that key's snapshot values. Recall clamps through the current
  target's taper (section 3.2), so even a stale value cannot leave the param's range.
- **Snapshot deleted.** Buttons whose source is that snapshot's `id` are dropped, and the face
  targets in parents that pointed at those buttons, by the prune of section 4.
- **Clock module removed.** The prune clears `clock` of every snapshot that named it, so the
  snapshot jumps; the file stays valid. Undo of the removal restores it.
- **Stepped control in a morph.** Jumps at half the morph length (section 3.3).

## 4. Nested targets (decision D5)

Any element may name a leaf at any depth: `source` modules are checked against
`composite::leaves(state, C)` (`composite.rs:105-111`), which covers every nested composite.

A face `button` may instead target a `button` on the face of a composite nested inside it, at any
depth. Pressing a button resolves a chain:

1. `press(C, key)`: read placement `key` of `C`'s panel.
2. Its source is a module action: `perform::fire(leaf, action)`; a snapshot: recall it on `C`
   (section 3.2); a face button `(D, key2)`: `press(D, key2)`.

**Cycle rule.** A face target `(D, key2)` on composite `C` is valid only when `D` is a strict
descendant of `C` (`composite::descendants`, `composite.rs:90-104`, without `C` itself) and
`D`'s placement `key2` is a `button`. Every hop goes strictly deeper in the composite tree, which
has no cycles and at most 8 levels (`composite.rs:180-191`), so a chain ends after at most 7
hops. "A fires B fires A" cannot be stored: `B` is inside `A`, and `A` is not inside `B`.
`composite::validate` (`composite.rs:139`) checks the rule, so a file that breaks it is refused at
load, and the press walker needs no visited set.

**Lifetime: one prune, in the forward edit only.** Several edits can leave a reference dangling:
removing a module, a composite, an exposure, a placement or a snapshot, and a `SetComposite`
that shrinks `members` (a module leaves the composite, so it is no longer in `leaves`). One
function, `composite::prune(state) -> Vec<Op>`, finds them. On the state after the edit it
returns one `Op::SetComposite` per composite that needs a change, computed from the innermost
composite out (dropping a button can orphan a parent's face target; nesting is at most 8,
`composite.rs:180-191`, so one pass in that order is complete). For each composite it:

- keeps an exposure-bound placement (knob, selector, slider, toggle, icons, jack) only while its
  key is still a control or port;
- keeps a `Module` source placement only while the module is in `leaves(state, C)`;
- keeps a `Face` target only while `(D, key2)` is still a strict descendant's `button`;
- keeps a `Snapshot` source only while a snapshot with that `id` exists;
- drops snapshot values whose key is no longer a control, and clears a snapshot `clock` that no
  longer names a `clock` module.

The editor commits a removal or a composite edit as `Op::Group[op, pruned SetComposites...]`,
built in one place: `PatchEditor::append` (`editor.rs:243-246`), through which every editor edit
that can remove or change a composite reaches the log. It checks every op it gets, including the
ops inside a `Group` at any depth: `remove_module` wraps `RemoveModule` in a `Group` when cue
references exist (`editor.rs:367-374`), and `edit` (`editor.rs:597-601`) and `restore_to`
commit `Group`s that can hold `RemoveModule` and `SetComposite`. When any op in it is one of
those two, `append` runs `composite::prune` on the state after the op and, if that yields
anything, logs the wrapped `Group` as one entry. The other log paths (`seed_from`, the
`SetParam` gesture paths, `editor.rs:300-425`) carry no such op. `apply` itself never prunes: applying a
`SetComposite` writes exactly the value it carries, so every inverse restores the exact prior
state in any order. That matters because `changed_composite_ops` restores composites in
`BTreeMap` id order (`state.rs:193-215`): if applying a parent's restore pruned against a child
not yet restored, undo would lose the parent's elements.

`Op::RemoveModule`'s own retain (`state.rs:325-329`) keeps only keys that are controls or ports,
so it would wipe every new composite-level placement (their keys come from `next_interface`) on
any module removal anywhere in the patch. It is narrowed to exposure-bound kinds: it drops a
placement only when its kind binds an exposure and that key is gone, and leaves the rest to the
prune ops in the group. `SetComposite`'s inverse stays its own restore (`state.rs:83-86`); the
group's inverse is the reverse of its parts' inverses, so undo restores the pruned composites
and then the edit.

Applying a log that leaves a dangling reference (a hand-edited file) is refused by
`composite::validate`, as any invalid composite is now.

Insert and duplicate remap leaf IDs through `remap` (`composites.rs:634`) and composite IDs
through the `groups` map (`composites.rs:628`); `source` modules, face-target composites and
snapshot clocks inside the inserted set go through the same maps.

## 5. Storage

### 5.1 Schema diff

`crates/core/src/panel.rs`:

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
+    CuePads,
+    Banks,
+    Transport,
+    Snapshots,
 }

 pub struct Placement {
     pub kind: Kind,
     pub x: f32,
     pub y: f32,
     pub width: f32,
     pub height: f32,
+    /// What a composite-level element reads or acts on. None for exposure-bound kinds and
+    /// for `snapshots`.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub source: Option<Source>,
+    /// Display, button and block elements: printed name. Exposure-bound kinds keep using the
+    /// exposure label.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub label: Option<String>,
+    /// Icons: one PNG per option, in option order. Empty: the built-in glyph set.
+    #[serde(default, skip_serializing_if = "Vec::is_empty")]
+    pub icons: Vec<Artwork>,
 }
+
+#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
+#[serde(rename_all = "snake_case", deny_unknown_fields)]
+pub enum Source {
+    /// A leaf at any depth; `action` is one `perform::fire` action for a button, None for a
+    /// display or a block.
+    Module { id: ModuleId, action: Option<String> },
+    /// A `button` placement on a strictly nested composite's face.
+    Face { composite: CompositeId, key: u64 },
+    /// The snapshot of this composite whose stable `id` this is.
+    Snapshot { id: u64 },
+}
```

`crates/core/src/panel.rs`, on `Panel` (`panel.rs:34-40`), for the compact face (section 6.3):

```diff
 pub struct Panel {
     pub width: f32,
+    /// Width of the compact face, in 30-point units, 180 or more and below `width`. None: the
+    /// whole face always shows.
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub compact_width: Option<f32>,
```

`crates/core/src/composite.rs`, on `Composite` (`composite.rs:42-59`):

```diff
     pub panel: Option<Box<crate::panel::Panel>>,
+    #[serde(default, skip_serializing_if = "Vec::is_empty")]
+    pub snapshots: Vec<Snapshot>,
 }
+
+#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
+#[serde(deny_unknown_fields)]
+pub struct Snapshot {
+    /// From `next_interface`; never reused, so a deletion shifts nothing.
+    pub id: u64,
+    pub name: String,
+    /// Control exposure ID -> value in the param's own units.
+    #[serde(deserialize_with = "unique_map")]
+    pub values: BTreeMap<u64, f32>,
+    /// 0 = jump.
+    #[serde(default)]
+    pub morph_beats: f32,
+    #[serde(default, skip_serializing_if = "Option::is_none")]
+    pub clock: Option<ModuleId>,
+}
```

Snapshots live on `Composite`, not `Panel`: the panel is "portable presentation only"
(`panel.rs:1`) and a snapshot changes sound.

Keys and validation:

- Exposure-bound kinds keep the exposure ID as key (`panel.rs:93-96`). Every other kind takes a
  fresh key from `next_interface` (`composite.rs:56`), so it never collides with an exposure; a
  placement key that is also an exposure ID must be an exposure-bound kind.
- `Panel::validate` (`panel.rs:81`): the kind decides the binding (`controls` for knob, selector,
  slider, toggle, icons; `ports` for jack; `source` absent for those and for `snapshots`, present
  for display, button and the three module blocks, with `Module { action: Some }` only on a
  button). `icons` non-empty only on `Icons`, each passing `Artwork::validate` (`panel.rs:51-79`).
  `label` at most 40 characters. Minimum sizes extend the table at `panel.rs:102-106` (slider
  horizontal 120x55, vertical 40x160; toggle 64x55; icons 18 per option x 55; display 120x60;
  button 54x40; blocks at their module-face sizes; the drawing slices' captures set the exact
  numbers). Footprint and overlap rules are unchanged.
- `composite::validate` (`composite.rs:139`), which has the patch: a `Module` source is in
  `leaves(p, id)` (already computed at `composite.rs:204`); a `Face` target is a strict
  descendant's `button` (section 4); a `Snapshot` source names an existing snapshot `id`; snapshot
  ids unique and below `next_interface`; snapshot names 1-40 characters,
  at most 16 snapshots, keys are controls, values finite, `morph_beats` in 0..=16, `clock` names a
  `clock` module. Icon PNG pixels count toward the 8-megapixel document budget
  (`composite.rs:221-234`).
- `panels::validate` (`panels.rs:19`), which has the registry: the kind/taper table of section 2;
  `icons` length equals the option count; a display source's kind has a display (dispatch at
  `displays.rs:397-403`, `fx_displays.rs:149-166`); a button action is one `button_actions`
  offers for that module (`perform.rs:325-350`); block sources have the matching kind.

### 5.2 Compatibility (decision D4)

Pre-1.0 rule: one format step, no migration code. Exactly:

- `CURRENT_SCHEMA_VERSION` goes from 9 to 10 (`crates/core/src/format.rs:26`);
  `OLDEST_SCHEMA_VERSION` stays 8 (`format.rs:27`). `check_schema` already accepts
  `OLDEST..=CURRENT` (`format.rs:125-137`), so it reads 8, 9 and 10 with no change.
- `schema_for` (`format.rs:31-49`) becomes three tiers, checked in order: v10 when a
  `SetComposite` uses any new kind, `source`, `label`, `icons`, `snapshots` or
  `compact_width`; else v9 when the
  existing `binds_cable_param || fans_in` predicate holds; else v8. A file that uses only v9
  features stays v9, and a file that uses none of them is written exactly as today.
- Every new field is `#[serde(default)]`, so v8 and v9 files deserialize as they are; that is the
  whole of the "migration".
- Composite packages: schema 3 when a composite uses any of the above, else 1 or 2 as today
  (`composites.rs:545-555`); `decode` accepts `1..=3` (`composites.rs:572`).
- A build without these changes refuses a v10 file with "newer schema" instead of a serde error.
  There are no users to protect beyond that, so there is no down-conversion and no per-kind bump.

## 6. Drawing

`draw_authored_face`'s placement loop (`lib.rs:5884`) becomes a `match p.kind`.

### 6.1 Per-parameter kinds

- `jack`, `knob`, `selector`: unchanged (`lib.rs:5894-5953`, `lib.rs:5981-6004`).
- `slider`: a face slider in the footprint's control rect, in two variants: horizontal (the kit
  look, `kit::slider`, `kit/controls.rs:571`) and a vertical fader drawn in the face style. Value
  path copied from `perform_view::param_value` (`perform_view.rs:1729-1740`: `to_norm`,
  `from_norm`, `set_param_gesture`), the route badge from `routing::selector_plug`
  (`routing.rs:1543`, private: the slider slice makes it `pub(crate)`), the label from
  `face_label` (`lib.rs:6063`). Which variant draws is a UI setting, not stored in the patch
  (decision D2); it goes once the owner picks.
- `toggle`: `kit::parts::toggle` (`kit/parts.rs:78`) painted in the control rect; a click sets the
  other option with `editor.set_param`, inspect as `stepped_selector` does
  (`routing.rs:1532-1536`).
- `icons`: `stepped_selector` (`routing.rs:1431`) gains an icon mode: same segments, hit IDs
  (`sel:{key}.{k}`) and plug. Built-in glyphs (empty `icons`): plain lines in the segment's ink
  (`displays::glyph`), the points from `displays::plot` over `displays::wave` (made `pub(crate)`
  by the icon slice) for sine, triangle, saw, square, and the LFO's sample-and-hold steps; sets
  exist for `osc.va` and `lfo` `waveform`. Kosta chose the plain lines over the `face::trace`
  display style on 2026-11-26, as #261 draws them. Author PNGs: decoded
  once with `panels::decode_art` (`panels.rs:13-18`) into a texture cache like the artwork one
  (`panels.rs:106`), drawn as an alpha mask tinted with the segment's ink, so one PNG works in
  both themes and in the selected and unselected states. An `icons` placement on a param with no
  built-in set and no PNGs is invalid.
- `panel_control_geo` (`lib.rs:5532`) maps slider, toggle and icons to `Geo::Select { rect }`
  so reveal, focus and pulse rings (`lib.rs:6007-6023`) work unchanged.

### 6.2 Composite-level kinds

- `display`: `displays::draw` takes `(id, info)` instead of `&Placed`. Checked at `266bb279`: the
  three display files read only `m.id` and `m.info` (29 field uses: 15 in `displays.rs`, 12 in
  `fx_displays.rs`, 2 in `drum_displays.rs`), and every function they pass `m` to (`param`,
  `displays.rs:365`; `live_param`, `fx_displays.rs:43`; the per-kind drawers) is in the same three
  files. The only outside caller of `displays::draw` is `lib.rs:3152`. The face calls
  `wants.display` and `wants.feeds` for the source when the footprint is on screen, as module
  faces do (`lib.rs:3146-3150`), then `displays::draw` in the footprint. Taps are by module ID,
  so a nested source works the same, and the same module shown on its own face and on the
  composite face asks once (`displays.rs:176-182`). The 16-tap budget (`facetap.rs:17`) is
  shared; past it the display shows its parameter-only picture, as module faces already do.
- `button`: a pad; press resolves the chain of section 4 and ends in `perform::fire(editor, view,
  leaf, action)` (`perform.rs:419`, private: the button slice makes it `pub(crate)`), so
  host-clock refusal and cue timing are inherited, or in a snapshot recall. Lit state:
  `cues::standing_now` (`cues.rs:408`) for cue buttons, the bank state for bank buttons, the last
  recalled snapshot for snapshot buttons; a face-target button shows the lit state of the button
  it ends on.
- `cue_pads`, `banks`, `transport`: call `draw_cues`, `draw_banks`, `draw_transport` with the
  source and the footprint rect; they already take `(id, rect)` (`lib.rs:4106-4115`,
  `lib.rs:3665-3675`, `lib.rs:3546-3554`). They key hit rects and egui IDs by module ID
  (`lib.rs:3584-3587`, `lib.rs:4146`), which would collide when the module's own face is drawn
  in the same frame, so each gains a key prefix argument: empty on the module face,
  `composite:{cid}:` on a composite face.
- `snapshots`: a row of pads, one per snapshot, in the cue-pad look; press recalls; the last
  recalled one is lit.

Plugin build: the CLAP editor draws the same rack through `kabl_ui::show`
(`crates/clap/src/lib.rs:795`), so every element appears there. A button's `run`/`restart` under
the host clock shows the existing refusal (`perform.rs:420-423`).

### 6.3 Compact face and expanded part

Owner request, 2026-11-24, after the Space captures: a composite face can be smaller, with the
rest behind an expand chip, as module faces do.

- **How module faces do it.** A module face shows its primary controls; the rest are listed in
  `Placed::hidden` (`rack/mod.rs:259`, filled at `rack/mod.rs:871`) and the face draws a "+N"
  chip (`lib.rs:3269-3276`). A click puts the module id in `ui_state.expanded`
  (`lib.rs:347`, toggled at `lib.rs:3334-3339`; one module at a time), which is session view
  state, not the document. Expanded, the off-face controls are laid out in an advanced area to
  the right of the face (`rack/mod.rs:776-871`, `Placed::adv` at `rack/mod.rs:254`) and the
  chip reads "Less"; the rack layout bounds do not change (test at `rack_editor.rs:1268-1276`).
- **Which placements are compact.** Panel height is fixed at 340 (`panel.rs:6`), and the module
  drawer grows to the right, so the fold is a width, not a height: `compact_width` on `Panel`
  (section 5.1). A placement whose footprint ends at or left of `compact_width` is in the compact
  face; one that starts at or right of it is in the expanded part; one that crosses it is refused
  by `Panel::validate`. A `jack` placement must be in the compact face: module faces draw every
  jack whatever is hidden (`draw_jacks` runs after the control loop, `lib.rs:3267`), so a cable
  always has an anchor, and refusing jacks past the fold keeps that true for composites without
  a second anchor rule. The chip has a reserved rect, 54x22 in the compact face's bottom-right
  corner (x from `compact_width - 62`, y from 310), and `Panel::validate` refuses a compact
  placement that overlaps it, so the chip never covers a control. Position decides, so no per-placement flag is stored and what the author
  sees on the design page is what the rack draws.
- **Drawing.** With `compact_width` set and the composite not expanded, the rack slot is
  `compact_width` wide (the three width reads at `rack_editor.rs:111`, `composites.rs:1091`,
  `lib.rs:2756` go through one `Panel::shown_width(expanded)`), the artwork is clipped to it,
  and a "+N" chip (N = placements in the expanded part) is drawn in its reserved rect with the
  module chip's look. Expanded, the whole face is drawn at the same origin, over its right-hand
  neighbours like the module drawer, and the chip reads "Less". The state is a
  `ui_state.expanded_composites: BTreeSet<CompositeId>`, session only, cleared with
  `expanded` so one face or module is open at a time.
- **Design page.** A draggable vertical fold line on the canvas, snapped to 30-point units, with
  "No compact face" to remove it; elements that would cross it are shown in the error colour
  until moved, and Apply is refused while any cross (the same rule as validate).

## 7. Composite design page (decision D8)

A dedicated view for designing one composite's face. It replaces the panel tool window
(`panels.rs:179`); there is one editor, not two.

**Entry.** Every "Edit face" opens the page for that composite: the default face header
(`lib.rs:5609-5613`), the authored face (`lib.rs:5843-5851`) and the composite list
(`composites.rs:1180`). Opening calls `panels::open` (`panels.rs:133-143`), which loads the draft
and labels. The page takes the rack area the way Perform's full view does
(`lib.rs:1217`); Esc or Close returns to the rack.

**Layout.**

- Top bar: composite name and instance, width (180-840 in 30-point steps), Light/Dark preview,
  Undo, Redo, Reload, Default face, Export package, Import package, Apply.
- Left, palette: the composite's controls and ports (each with its kind chooser, only the kinds
  section 2 allows), then "Displays" (leaves with a display, nested ones as `Composite / Module`),
  "Buttons" (each leaf's `button_actions`, `perform.rs:325-350`; each nested composite's face
  buttons; each snapshot), "Blocks" (cue pads, banks, transport per leaf, and the snapshot strip).
  Placed items show a "placed" chip, as now (`panels.rs:353-411`).
- Centre, canvas: the face drawn by `draw_authored_face` from the draft, live (displays animate,
  lit states follow), at a fit-to-view zoom, with the header and footer bands and the 6-point
  grid shown. Controls on the canvas do not take input: the page's own layer of footprint
  handles sits above them.
- Right, inspector for the selected element: kind, source or exposure, label, x, y, w, h fields
  (the current `num_field`, `panels.rs:110-132`), icon PNGs per option for `icons` (Import… per
  option, using `import_art`, `panels.rs:53`), and the face artwork (light and dark PNG) when
  nothing is selected.
- Bottom, snapshots: list with name, morph beats, clock; Capture (new from the current values),
  Re-capture, Rename, Delete, Recall (to audition).

**Editing actions.** Add an element by clicking a palette item (placed at the first free spot of
its default size) or dragging it onto the canvas. Select by click; move by drag; resize by corner
handles; all snap to 6 points. Delete with the Delete key or the inspector. Change kind or source
in the inspector. Arrow keys nudge by 6 points. While a drag would overlap, leave the face or go
under the minimum size, the footprint is drawn red with the `Panel::validate` message, and the
drop is not applied (the element returns to where it was). Apply is disabled while
`Panel::validate` or `panels::validate` fails, with the message shown, as the banner does today
(`panels.rs:189-191`).

**What the draft holds and what Apply writes.** Today the draft is a `Panel`
(`panels.rs:99-107`, `panels.rs:133-143`). Snapshots live on `Composite` and new keys come from
`Composite::next_interface`, so the draft becomes a `Draft { base: Composite, panel: Panel,
snapshots: Vec<Snapshot>, next_interface: u64 }`: `base` is the composite as it was when the page
opened or last reloaded, and the other three are the parts the page edits. New element keys and
snapshot ids are taken from the draft's `next_interface`. Apply builds the composite to write
from the *current* composite with exactly those three fields replaced, so members, exposures and
everything else stay as the document has them, and writes it with `composites::set`
(`composites.rs:434-438`) through `PatchEditor::append`, which prunes (section 4). Apply refuses when the
current composite's members, controls or ports differ from `base` (the banner below), because
the draft's keys were validated against `base`. Nested composites the face depends on (face
targets, nested sources) may also have changed since the page opened, so Apply first runs
`composite::prune` on the composite it would write: when that would drop anything, Apply lists
the elements it would drop and asks before applying; when it would drop nothing, it applies.

**Undo.** Two levels, on purpose:

- Inside the page, every finished gesture (drop, resize, field commit, add, delete, snapshot
  edit) pushes the previous draft onto a page history; Ctrl+Z and Ctrl+Shift+Z and the top-bar
  buttons walk it. A history entry does not copy image bytes: the page keeps an append-only image
  pool for its session (each distinct PNG once, up to 128 KiB each, `panel.rs:7`), and an entry
  stores the panel, snapshots and `next_interface` with each `Artwork` (face art and icons)
  replaced by its name and pool index. An entry is then a few kilobytes; the history keeps at most
  100 and is dropped, with the pool, on Apply, Reload or close.
- Apply writes the draft as one project undo step, as now (`panels.rs:454`,
  `panels.rs:484-500`). Per-gesture project steps are avoided
  because `Op::SetComposite` stores the whole composite (`op.rs:55-58`) with up to 128 KiB per
  image (`panel.rs:7`) in the log on every step.
- Snapshot Recall from the page is a normal project edit (section 3.2), not a draft change.
- Close or Esc with an unapplied draft asks: Apply, Discard, Keep editing.
- If the composite changes under the page (project undo, a remove), the page shows a banner
  "The composite changed" with Reload; Apply then refuses until Reload, so a stale draft never
  overwrites the newer composite.

**Relation to `panels.rs`.** The page reuses `panels::View`'s labels, error and texture
cache (`panels.rs:99-107`), `open` (`panels.rs:133`), `validate` (`panels.rs:19`), `decode_art`,
`import_art`, `read_package`, `export` (`panels.rs:13-98`) and the Apply and Reload logic
(`panels.rs:484-506`). The tool-window body (`panels.rs:179-560`) is removed in the same slice.

## 8. Owner decisions

All answered by Kosta on 2026-11-24. Each replaces the recommendation the earlier draft made.

**D1. What "cues of their own" means.** Decision: **B, snapshot buttons**. "Cues of their own":
the composite stores named sets of its exposed control values; a button jumps or morphs to one.
Design: section 3. Option A (fire an inside `cues` module) is kept as a button source because
it costs nothing beyond the button.

**D2. Slider look.** Decision: "I judge based on screenshots, probably vertical". The slider slice
ships both a horizontal (kit) and a vertical fader variant behind a UI switch, with captures of
both; Kosta picks, and a follow-up removes the other variant and the switch.

**D3. Icon sets.** Decision: **B**. Built-in waveform glyphs plus an author-supplied PNG per
option in the package, validated as panel artwork (`Artwork::validate`, `panel.rs:51-79`).

**D4. Schema rollout.** Decision: "we have 0 users so we can break stuff without actually
breaking it." One format step for everything new (v10 and package schema 3), no migration code
beyond serde defaults and the existing version range check (section 5.2).

**D5. Sources in nested composites.** Decision: **A, recursive**. "A button on a composite fires
a button on a composite inside the composite, etc." Any element may target any leaf at any
depth; a face button may target a nested composite's face button, and so on down. Chain and cycle
rule: section 4.

**D6. Composite controls on the Perform page.** Decision: **B**. "Pin to Perform" on composite
face controls (slice 10).

**D7. Whole module blocks on a face.** Decision: **B**. Cue pad grid, transport, bank strip and
snapshot strip are each offered as one element, as well as single buttons.

**D8. Placement.** Decision: **B, as a "composite design page"** rather than an inline preview: a
dedicated view to design a composite's face (place, drag, resize, pick element kind and target,
artwork), opened from the composite. Design: section 7.

**Compact face (owner request, 2026-11-24).** "Perhaps we can do the smaller with expanding
part thing here or allow it for these": allowed for any composite face; design in section 6.3,
slice 11. Factory composites such as Space (`docs/ROADMAP.md`, CB1) set `compact_width` in
their own slices.

## 9. Slice plan

Each slice branches from `origin/master`, keeps existing patches sounding and looking the same,
and is checked by `cargo +1.99.0 clippy --workspace --all-targets -- -D warnings`,
`cargo +1.99.0 test --workspace` and green CI (`gh pr checks`, which also runs `cargo kabl test`
on the SDK template). Captures are light and dark, of a composite face (or the design
page), under the job scratch dir; anything audible is an offline render or runs on the
`kabl_agents` sink. Order matters: each slice needs the ones before it.

1. **Stored form (v10).**
   Scope: `crates/core/src/panel.rs`, `crates/core/src/composite.rs`, `crates/core/src/format.rs`,
   `crates/core/src/state.rs`, `crates/ui/src/composites.rs`, `crates/ui/src/panels.rs`, tests.
   Done means: (a) the section 5.1 diff with serde defaults; (b) `composite::validate` and
   `Panel::validate` enforce section 5.1, with one test per rule, including a face target on a
   non-descendant and on a non-button refused, and a snapshot key that is not a control refused;
   (c) `panels::validate` accepts knob, selector, jack exactly as before and refuses every new
   kind with "not supported yet" (later slices lift this per kind); (d) a v8 and a v9 fixture
   load and re-save byte-identical; (e) `schema_for` writes v10 for a log with a snapshot and for
   one with a new kind, v9 for a log with only a fan-in port, v8 otherwise (three tests); package
   schema 3 likewise, `decode` accepts 1..=3; (f) the retain at `state.rs:325-329` is narrowed to
   exposure-bound kinds and `PatchEditor::append` prunes after every op, including ops inside a
   `Group` (a `remove_module` with cue references, `edit`, `restore_to`; one test each):
   removing a module drops
   `source` placements, snapshot values and snapshot clocks of that module, while a `display`,
   `button` and `snapshots` placement whose source is another module survives (test); undo
   restores the exact prior state, tested on a fixture whose parent composite has a smaller id
   than its nested child (so `changed_composite_ops` restores the parent first) with a parent
   display sourced from the child; (g) removing a composite, a button placement or a snapshot,
   or shrinking `members`, drops the dependent placements and face targets in every parent, and
   undo restores them exactly (tests, same parent-id-below-child fixture); applying a
   `SetComposite` alone never changes another composite (test); (h)
   `remove_exposure` drops the key from every snapshot; (i) insert and duplicate remap `Module`
   sources, `Face` composites and snapshot clocks (test); (j) deleting a snapshot leaves buttons
   on the other snapshots pointing at the same snapshots (test on ids).
2. **Composite design page** (existing kinds only).
   Scope: `crates/ui/src/panels.rs`, `crates/ui/src/lib.rs` (entry points, view switch),
   `crates/ui/src/composites.rs` (entry point), tests.
   Done means: every "Edit face" opens the page (test through recorded hit rects); the tool
   window is gone; drag moves and corner drag resizes with 6-point snap (test); an overlapping or
   out-of-bounds drop is drawn red and not applied (test); page undo and redo walk the draft
   history and Apply makes exactly one project undo step (test); Apply replaces only the panel,
   snapshots and `next_interface` of the current composite (test); a history entry holds no
   image bytes (test on the pool: re-adding the same PNG does not grow it); close with an
   unapplied draft asks; a composite changed under the page blocks Apply until Reload (test);
   captures of the page
   with a composite selected and with a red drop.
3. **Slider, both variants.**
   Scope: `crates/ui/src/lib.rs`, `crates/ui/src/routing.rs` (`selector_plug` visibility),
   `crates/ui/src/panels.rs`, tests.
   Done means: slider accepted on continuous params and refused on stepped ones (test); offered
   in the page's kind chooser; dragging changes the leaf param through its taper with one undo
   entry per gesture (test through recorded hit rects), in both variants; route badge shows; the
   variant switch is a UI setting, not saved in the patch (test: saving with either setting gives
   the same file); captures of the same face with the horizontal and with the vertical variant.
4. **Toggle.**
   Scope: as slice 3 without `routing.rs`. Done means: toggle accepted only on two-option stepped
   params (test, e.g. `vca.exponential`, `arp.latch`); click flips the value, undoable (test);
   captures.
5. **Icon selector.**
   Scope: `crates/ui/src/routing.rs`, `crates/ui/src/displays.rs` (`wave` to `pub(crate)`, glyph
   helper), `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, tests.
   Done means: `icons` with built-in glyphs accepted on `osc.va.waveform` and `lfo.waveform` and
   refused elsewhere without PNGs (test); with PNGs accepted on any stepped param when the count
   matches the options and refused otherwise (test); a bad PNG refused with the
   `Artwork::validate` message (test); each segment's hit rect selects its option (test); the page
   inspector imports a PNG per option; captures of built-in glyphs and of PNG icons at zoom 1 and
   the smallest face zoom, light and dark.
6. **Display.**
   Scope: `crates/ui/src/displays.rs`, `crates/ui/src/fx_displays.rs`,
   `crates/ui/src/drum_displays.rs` (signature `(id, info)`), `crates/ui/src/lib.rs`,
   `crates/ui/src/panels.rs`, tests.
   Done means: the page's Displays list shows leaves with displays, nested included (test); a
   display placement on a closed composite asks the engine for the leaf's tap (test on `wants`
   output); a leaf two composites deep draws; module-face displays unchanged (existing display
   tests pass); captures with audio running (offline render of the frame, no sound on the default
   output).
7. **Buttons: module actions and nested chains.**
   Scope: `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, `crates/ui/src/perform.rs` (`fire`
   to `pub(crate)`), tests.
   Done means: the page's Buttons list shows the cue, bank and clock actions of all leaves and
   the face buttons of nested composites (test); pressing a cue button launches that cue on the
   inside `cues` module (test on the queued launch); a button on the outer face targeting a
   button two composites down fires the innermost action (test); host-clock refusal holds (test);
   lit state follows the cue or bank state, through a chain too; Apply on the design page after
   a nested composite lost a button the draft targets lists that element and asks before
   applying (test); captures.
8. **Engine morph of any parameter.**
   Scope: `crates/engine/src/morph.rs`, `crates/engine/src/patch_engine.rs`,
   `crates/engine/tests/`, `crates/ui/src/cues.rs`, `crates/ui/src/app/audio.rs`,
   `crates/ui/src/app/callback.rs`, `crates/ui/src/app/frame.rs`, `crates/ui/src/lib.rs`
   (`live_macro`), `crates/ui/src/perform_view.rs`, `crates/clap/src/lib.rs`, tests.
   Done means: section 3.3; every existing cue-macro test passes unchanged (`tests/cue_macros.rs`);
   new engine tests: a continuous exponential param morphs evenly in knob travel; a stepped param
   holds until half the length and then jumps, never visiting an option in between; an explicit
   `from` is used; a value write releases the morph of a non-macro param; a param the module does
   not smooth moves with no step larger than one block's `RAMP_MS` ramp (test on rendered
   values); a 32-target recall sent as 4 parts starts every target in the same block, and the
   targets keep their old values while only some parts have arrived (test splitting the parts
   over two `drain` calls); 32 targets plus 16 macros run together; every abandon path of
   section 3.3 (clock gone, full pending table, full driven table, replaced `seq`, reset, cancel)
   leaves a recall target at `to` (one test per path) and leaves a cue macro where it was; a
   reset with 64 pending, 64 driven and 32 staged recall targets lands all 160 (test);
   `size_of::<Command>()` is not larger than before (test);
   `drain` and `drive_morphs` run under `assert_no_alloc` (as the engine tests already do, e.g.
   `crates/engine/tests/cue_macros.rs`) with no allocation.
9. **Snapshots.**
   Scope: `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, `crates/ui/src/composites.rs`,
   `crates/ui/src/control.rs` (`deliver` takes `ui_state.morphs`, `sync` gets a skip list), a new
   `crates/ui/src/snapshots.rs`, tests.
   Done means: the page captures, re-captures, renames, deletes snapshots and sets morph beats
   and clock (tests, each one draft change); recall is one project undo step that sets every
   control whose value changes, and only those (test); with a morph length and a clock, one
   `deliver` pushes the values `sync` held for other targets, then the parts, and no `ParamSet`
   for a morphing target, then or on later frames (test on the pushed `ToAudio` sequence); with
   a Load, a compile in flight, a compile waiting, the `MAX_HELD` fallback or `held_actions`
   without room for every part, the recall jumps: values go, no part is queued (one test each);
   an un-exposed control is not touched by a later recall (test); snapshot buttons and the snapshot strip recall and light the last one
   (test); a nested button chain ending on a snapshot recalls it (test); offline render of a
   recall with morph shows the parameter moving over the set beats; captures of the strip and
   the page's snapshot list.
10. **Whole blocks and Pin to Perform.**
    Scope: `crates/ui/src/lib.rs`, `crates/ui/src/panels.rs`, `crates/ui/src/perform.rs`, tests.
    Done means: `cue_pads`, `banks`, `transport` elements draw through `draw_cues`,
    `draw_banks`, `draw_transport` with a key prefix, and their hit keys stay unique when the
    module's own face is drawn in the same frame (test); pressing a pad on the composite face
    acts as on the module face (test); right-click on a composite face control offers "Pin to
    Perform", which pins the leaf param under the exposure label (test), also for a control whose
    leaf sits in a nested composite (test), and `pin_source` names the composite path
    (`Composite / Nested · Module #id · param`); removing the exposure leaves the pin on the leaf;
    captures of a face with each block and of the Perform card.
11. **Compact face.**
    Scope: `crates/core/src/panel.rs`, `crates/core/src/format.rs`, `crates/ui/src/lib.rs`,
    `crates/ui/src/rack_editor.rs`, `crates/ui/src/composites.rs`, `crates/ui/src/panels.rs`,
    tests.
    Done means: `Panel::compact_width` with its serde default, `Panel::validate` refuses a
    width outside 180..`width` or off the 30-point grid, a placement that crosses the fold, a
    `jack` placement past the fold and a compact placement overlapping the chip rect (one test
    each), and `schema_for` writes v10 for a panel that sets it (test); collapsed, the
    rack slot is `compact_width` wide, only compact placements draw and take hits, every jack
    draws and anchors its cables at the same point collapsed and expanded (test), and the chip
    draws in its reserved rect with the count of the others (test); a click on the chip draws the whole face at the same
    origin with "Less", leaves the rack layout bounds unchanged (test, as
    `rack_editor.rs:1268-1276`) and opening it closes an expanded module (test); a panel without
    `compact_width` draws as before (test); the design page's fold line sets and removes it as
    one draft change and marks crossing elements (test); captures of a composite collapsed and
    expanded, and of the design page with the fold.
