# First-run journey and learning system: design (investigation 2)

2026-11-23 · read-only on `origin/master` @ `89d9c1be` · pivot brief sections A and B.
Screenshots of a true first run (fresh `HOME`/`XDG_*` under scratch, release build, Xvfb 1440×900, audio on the silent `kabl_agents` sink, verified with `pactl list sink-inputs`): `.claude/tmp/coordinator/reports/firstrun/*.png` (absolute: `/secondary/Programming/Github/kabl/.claude/tmp/coordinator/reports/firstrun/`).

## 1. Current state

### 1.1 Standalone, first launch (what actually happens)
1. `kabl-ui` with no `--patch` builds the code-defined start patch **Pluck line** (`crates/ui/src/first_run.rs:46-183`: clock → seq → VA osc → SVF → VCA ← ADSR, delay, reverb, out; the ADSR also opens the cutoff by +55 %) and **opens the Sounds page on top of it** (`crates/ui/src/app/startup.rs:234-246`). Its clock runs from creation (`crates/modules/src/builtins/clock.rs:136`), so a sequence plays at once, but the user is looking at a 129-sound list and cannot see what is playing (`01-launch.png`).
2. The tour starts unless `first_run_done` is set (`startup.rs:238`, `tour.rs:52-55`; flag in `library.json` under `$XDG_DATA_HOME/kabl`, `library.rs:144-157`, `library.rs:285-296`). Step 1 is a toast "Press play on any sound to hear it." The toast sits over the detail column and hides its first lines ("In the rack: Pluck line", `01-launch.png`).
3. Pressing a row's ▶ previews it: the clip is rendered offline and mixed **on top of the still-running Pluck line** (`sound_preview.rs:1-12`; `02-preview-acid-line.png`: "sequence running", output peak −2.8 dB). Preview does **not** advance the tour; only opening a sound does (`tour.rs:168-173`). "Preview when selected" is off by default (`library.rs:153`).
4. Load sound (Acid Line): the view jumps to the rack and **three toasts stack**: the tour's "Sound flows along the glowing cables", the browser's "opened 'Acid Line' (stopped, press Start)" while the tour has just started its clocks (`tour.rs:176-185`), and "file operation in progress" (`03-loaded-acid-line.png`). The rack is a flat 62-cable patch at 50 % zoom; the routing drawer lists all cables. Peak −0.3 dB.
5. After 4 s, step 3 dims the rack and spotlights the first `filter.*` it finds (`tour.rs:198-218`, `first_run.rs:186-192`): one of Acid Line's three ladders, with generic text "the teal arc is the envelope opening the filter on every note", written for Pluck line (`04-tour-step3.png`). The scrim blocks every other click (`tour.rs:345-364`).
6. Step 4 "Make it yours: drag a cable end out of its jack … or browse more sounds" (`05-tour-step4.png`), then the tour ends for good (`tour.rs:118-126`). Confirmed: `first_run_done: true` was written and the second launch shows no tour (`08-second-launch.png`).
7. Every launch, including the second, starts on Pluck line again, not on the last sound (`startup.rs:234-246`); recents are kept.

**What works:** sound at once; a fast library with shelves, macro names per sound, search, previews at matched loudness, description, waveform (`02-preview-acid-line.png`); the Perform view of a preset is clear and already prints what each macro moves (`06-perform.png`, `perform_view.rs:1508-1545`); Explain "?" per card (`perform_view.rs:680-703`); the legible Pluck line rack (`09-start-patch-rack.png`); a Learn tab with three working recipes (`07-learn-tab.png`).

**What blocks a newcomer:**
- **No way to play a keyboard sound without a MIDI controller.** There is no computer-keyboard input and no on-screen keyboard (grep over `crates/ui/src`, `crates/standalone/src`); keyboard sounds sound only from MIDI or the fixed-length audition "Play C4" button in Sounds (`browser.rs:9-10`, `2450-2560`). The Bass, Lead, Keys, Pad, Pluck and Voice shelves hold 85 of the 129 sounds (sidebar counts, `01-launch.png`); these are keyboard sounds.
- The first patch the user is guided through is **not the start patch** but whatever they open first, often a large flat rack (Acid Line: 62 cables). The clean Pluck line rack is never shown by the tour.
- The tour's knob text is fixed to Pluck line but applied to any filter in any patch; its scrim blocks input; its step 1 ignores preview.
- Conflicting toasts on load (stopped vs. running); preview plays over the running start patch.
- Pluck line has **no macros**: its Perform page is empty ("No pinned controls", `10-start-patch-perform.png`), so the first sound cannot teach the first meaningful change.
- Audio device choice is a small dropdown at the right end of the status bar (`app/frame.rs:421-440`) and is **not remembered**; MIDI auto-connects to the first non-"Through" port (`app/midi.rs:236-253`), choice not remembered; buffer size only via `--frames` (`startup.rs:69-77`). `style::Prefs` has no audio or MIDI fields (`style/load.rs:562-581`).
- Nothing says what to do after the four steps; Learn is a drawer tab icon.

### 1.2 CLAP plugin, first open
- Starts on `kabl_standalone::default_patch()`, a MIDI keyboard voice (`crates/clap/src/lib.rs:830`, `crates/standalone/src/lib.rs:136`), editor 1180×680 (`clap/src/lib.rs:1005`), same UI code (`kabl_ui::show`, `clap/src/lib.rs:776`). `UiState::default()` leaves `first_run` `None`, so **no tour and no browser** in the plugin (`clap/src/lib.rs:853-858`). Library loads lazily on first editor frame (`clap/src/lib.rs:645-647`). Sound needs host MIDI; no on-screen keys. Not run in a host for this investigation (code reading only).

### 1.3 Help, explanation, learning pieces that exist
- `help.rs`: authored text per module/param/jack from `ModuleInfo.explain` (`help.rs:1-6`).
- `explain.rs`: "What does this control change?" from the live graph, macros list real destinations, Show/Back navigation (`explain.rs:1-16`, `destinations` :319, `path_to_output` :367).
- `inspect.rs`: signal taps and the "Why no sound?" aid (`inspect.rs:1-7`). `compare.rs`: reference capture and Restore as one undo step (`compare.rs:1-7`).
- `recipes.rs`: exactly three listening recipes (Pluck to pad, Filter movement, Interlocking) hard-coded in Rust, each on a factory teaching patch; manual Next only, no completion detection; view-only; detach on another document (`recipes.rs:1-8`, `76-274`, `346-382`). Rules in `docs/signal-inspection/design.md:119-134`.
- `tour.rs` (534 lines) + `first_run.rs` (219): the first-run tour and three start-patch candidates (`docs/first-run/*.mp3` are their listening renders).
- `UiState::hits`: every widget's rect under a stable key each frame (`lib.rs:225-228`, `record` :594); the tour and all headless tests anchor on it.
- Unused remnants: `ModuleInfo.lesson: Option<LessonId>` (`crates/modules/src/info.rs:183,194`), never set by any module; the `learn` crate and `lessons/` scaffold are gone (`docs/STATUS.md:825` says "lessons/ Empty, scaffolded. Content is v5 scope").

### 1.4 The original learning vision in docs
- `docs/planning-prompt.md:56`: "`learn`: lessons and unlock flags. Timing unclear."
- `docs/PRODUCT-PLAN.md:47-53` principles: "A working sound precedes a lesson. No forced tutorial or feature unlocking"; "Musical controls reveal their real targets"; "Complexity can be opened, inspected, and edited"; "Current patch state is authoritative". `:66`: "the main differentiation is largely unbuilt".
- `docs/design/redesign/BRIEF.md:69`: same principles carried into the redesign.
- `docs/confusions.md:3`: "Owner's beginner confusions … Lesson source material for v5." Still empty.
- `docs/product-research/briefs/D03.md:96-98`: "Exactly three optional listening recipes. No mandatory tutorial, unlocks, scores, runtime LLM or general curriculum engine." The pivot reverses only the last clause (section 3.8).

### 1.5 `docs/ACCEPTANCE-V1.md` vs. the code
- §1 (`:7-9`): "The first-run overlay appears and offers a starter sound. Accept the overlay. The default patch `pluck-line` loads and plays a plucked line on a key press." Reality: no overlay to accept; Pluck line is a self-running sequence, not key-played; the tour is toasts plus one spotlight.
- §2 (`:12`): "Open settings (gear), pick your audio device and buffer size." Reality: device is a status-bar dropdown, buffer size has no UI. Both lines need rewriting with slice L3.

## 2. First-run design

One system: the first run is **lesson 00 "Start here" of the same lesson engine** (section 3), offered once, never blocking. `tour.rs` is retired into it, so there is one onboarding path, not two. Standalone and plugin differ only in the setup strip (screen A) and a few plugin-only lesson steps.

Targets: sound within 2 s of launch; a note the user plays within 30 s, without a MIDI controller; a sound found, changed and saved within 5 minutes.

### Screen A: "Sound check" strip (standalone only; first launch, and whenever audio is Unavailable)
- A slim strip under the top bar, not a modal: `Output: System default ▾ · MIDI: <port | none> ▾ · Play from: computer keys ✓ · [Test sound] · [Hide]`.
- Reuses the status-bar output dropdown (`crates/ui/src/app/frame.rs:421-440`) and the MIDI menu (`crates/ui/src/lib.rs:1800`). New: remember the choice (`style::Prefs`, `crates/ui/src/style/load.rs:562-581`, gains `audio_device`, `midi_input`; today neither is remembered) and the test sound (existing `Command::Preview`, as `recipes.rs:393-402`).
- Plugin: no strip; the host owns devices and MIDI. The card's first line says "Notes come from your DAW track" (plus "Tempo from host" when `host_clock`).
- Audio Unavailable (`app/startup.rs:91-95`): the strip shows the existing Retry and output choice in plain words. Nothing else is blocked.

### Screen B: the start patch, already playing, and the Start card
- Today the standalone opens Sounds on top of the start patch (`startup.rs:234-236`). Proposed: open on the **Rack with Pluck line playing** (its clock runs from creation, `crates/modules/src/builtins/clock.rs:136`), browser closed, and a **Start card** docked bottom-right (no scrim):
  - "This is *Pluck line*, playing now. Every sound in kabl is a patch of modules and cables like this one."
  - Four actions, any order, each one lesson-00 step: **Play it yourself** · **Find other sounds** · **Change it** · **See how it works**. Below: "Learn kabl: 8 lessons" and "Not now" (Esc). "Not now" never brings it back automatically; the Learn tab always has it.
- Plugin: opens on `default_patch()` (a MIDI keyboard voice, `crates/clap/src/lib.rs:830`, `crates/standalone/src/lib.rs:136`), silent until the host sends notes. Same Start card on the first plugin instance ever (progress shared via `library.json`). A plugin never starts a sequence by itself when inserted into a project.

### Step "Play it yourself"
- New input (both builds): **computer-keyboard notes** (A-row white keys, W-row black keys, Z/X octave; only when no text field or popup has focus, the guard the tour already uses, `tour.rs:152-155`) and a **collapsible on-screen key strip** under the rack/Perform. Notes take the MIDI path: standalone the notes ring (`startup.rs:66-68`); plugin a small UI-to-audio note ring merged with host MIDI. Today there is none: a keyboard sound is silent without a controller, apart from the fixed-length audition button in Sounds (`browser.rs:2450-2560`) and recipe test notes.
- Pluck line plays itself, so this step says "This one is a sequence: it plays itself. Keyboard sounds play from your keys" and loads a keyboard sound in the next step.
- Done when: notes played (`status_voices` > 0, `lib.rs:416`, or a key event from the new input).

### Step "Find other sounds"
- Opens Sounds (scene tabs, `lib.rs:1712-1726`) on a shelf (`library.rs:33-35`), with **Preview when selected** on (pref `preview_on_select`, `library.rs:153`, off by default; decision D3). Selecting previews (`sound_preview.rs:1-12`), Open loads.
- Done when: a library sound is opened (`doc.origin` changed, as `tour.rs:168-173` checks) and a note is played on it. One line after loading: "Keyboard sounds play from your keys; pieces have Start" (rule from `browser.rs:9-10`).

### Step "Change it" (the first meaningful change is a macro)
- All 100 factory presets carry a `macro` module (grep over `patches/presets/*/log.jsonl`). The step opens **Perform** and rings the first macro card (hit key `pcard:…`, `perform_view.rs:585`).
- Done when: that macro moved (`macro_live`, `lib.rs:266`, or a `SetParam` on it in the op log).
- **"What did that do?" — mostly exists today.** Each Perform macro card already prints its destinations under the knob ("Ladder · Cutoff", "+2 more", full list in the tooltip; `macro_moves`, `perform_view.rs:1508-1545`; screenshot `06-perform.png`), `explain::destinations()` (`explain.rs:319`) gives the real cables with signed depth and bypass, one hop through jacks (`explain.rs:7-9`), and the card's "?" opens the full explanation (`perform_view.rs:680-703`). Gaps: the line names a module kind, not which one ("Ladder" when Acid Line has three ladders), shows no depth, and nothing in the rack lights up. New and small: while a macro is turned, expand the list with depth and module number ("Ladder #7 Cutoff +40 %") and pulse those knobs' rings in the rack. Same graph data, no second model (principle 2, `docs/PRODUCT-PLAN.md:48`).
- The start patch has **no macro module** (`first_run.rs:127-183`; its Perform page says "No pinned controls", screenshot `10-start-patch-perform.png`), so lesson 00 cannot teach macros on it. Either Pluck line gets two macros (Brightness → cutoff + envelope route depth, Space → delay/reverb mix) at values that leave its sound identical, or this step loads a preset. Decision D7.
- Then **Show** on one target (`explain::go`, `explain.rs:165`: pans, expands an off-face control, flashes; Back returns).

### Step "See how it works"
- Selects Output, follows the cable back (`explain::path_to_output`, `explain.rs:367`), inspects one tap (`inspect.rs:1-7`). This is lesson 3, step 1.

### "What next"
- After any two steps the Start card shrinks to a top-bar chip "Learn kabl · 1/8" that opens the Learn hub. Nothing else nags.

### Safety
- A step never edits the patch without the user's own action. Opening a lesson patch goes through the normal unsaved-changes question (`recipes.rs:346-351`). The user keeps every edit.

## 3. Learning system architecture

**Three lines:** a lesson is a TOML file (steps, target patch, anchors, completion predicates, listening prompts) embedded in the binary. One runner in `kabl_ui` evaluates the predicates each frame against the real `PatchState`, the op log and `UiState`, rings anchors from the existing `hits` registry, and never edits the patch itself. Progress lives in `library.json`, shared by standalone and plugin; each lesson ends in the user's own patch saved through ordinary Save As.

### 3.1 Where it lives
- `lessons/<nn>-<slug>.toml` at the repo root (the empty `lessons/` scaffold of `docs/STATUS.md:825` was planned for exactly this), embedded with `include_str!` from a list generated in `crates/ui/build.rs` (which already packs `patches/`; `library.rs:280-283`). Embedded means plugin and standalone get them with no new install path.
- Lesson patches are ordinary factory sounds under `patches/lessons/…`, generated by a test like the recipe patches (`crates/ui/tests/library.rs:1031-1069`; `docs/signal-inspection/design.md:121-123`). Lessons can also use any shipped sound (`factory:presets/lichen`).
- Code: `crates/ui/src/lessons/{mod.rs (types, parse), run.rs (runner, predicates), panel.rs (hub and step card), mark.rs (anchor ring)}`. `recipes.rs` (705 lines) and `tour.rs` (534 lines) are replaced; the three recipes become lesson steps (data), their Show / Inspect / Compare / test-note buttons become generic step actions.

### 3.2 Format (TOML via serde; `toml` and `serde` are already deps, `crates/ui/Cargo.toml:27-29`)
```toml
id = "02-change"            # stable progress key; never renamed
stage = 2
title = "Change sounds"
summary = "Turn a macro, see what it really moves, change it at the source."
minutes = 8
where = "both"              # both | standalone | plugin
start = { sound = "factory:presets/sock-puppet" }   # or { current = true } or { from = "04-voice" }

[bind]                      # resolved when the lesson patch opens
macro = { kind = "macro", id = 12 }          # authored patch: explicit id + kind, never a name
# on the user's own patch: { kind = "filter.*", added_in_step = 3 }

[[step]]
text = "Open Perform. Turn Brightness and listen to the top end open."
listen = "The attack gets brighter before the sustain does."
anchor = "pcard:{macro}.*"                   # a hits key; {binding} substituted, * glob
do = [{ scene = "perform" }]                 # optional buttons: scene, play_note, show, inspect, compare
done = [{ macro_moved = "{macro}" }]          # all must hold; empty = Next only

[[step]]
text = "Press ? on Brightness: these are the real knobs it turns."
anchor = "pexplain:{macro}.*"
done = [{ explain_open = "{macro}" }]

[end]
keep = "Save your version (Save As). The factory sound stays as it was."
suggest_name = "My bright puppet"
unguided = "On your own: make Brightness close the filter instead (negative depth)."
```
- Predicates are a **closed serde-tagged Rust enum**, not an expression language. All of these are observable today:
  - Patch (`PatchState`): `has_module{kind}`, `connected{from,to}`, `route{from,to_param}`, `param_moved{target,by}` (against the value at step start, via `routing::base_value` as `tour.rs:128-133` does), `reaches_output{module}` (`explain::path_to_output`), `composite_exists`, `exposed{composite,param}` (`composites.rs:245`).
  - Op log since step start (`PatchEditor::log()`, `editor.rs:205`; ops in `crates/core/src/op.rs:56-94`): "the user added / connected / set X", as distinct from "the state is X".
  - Document: `opened{sound}` / `opened_any` (`UiState::doc`, `lib.rs:180`; `editor.instance()`), `saved_as_user` (`DocOrigin::Library("user:…")`), `favorited` (`library.rs:145`).
  - View: `scene{rack|perform|sounds}` (`perform_open`, `browser_open`), `explain_open{subject}` (`UiState::explain`), `inspecting{module,port}` (`UiState::inspect`), `compare_restored` (`UiState::compare`), `drawer_tab`.
  - Sound: `notes_played` (`status_voices`, `lib.rs:416`, plus the new key input), `preview_played`, `audible` (`record::Meter` above −60 dBFS, `lib.rs:407`), `clock_running{id}` (`lib.rs:261`), `cue_launched` (`last_cue`, `lib.rs:269`), `macro_moved` (`macro_live`, `lib.rs:266`), `cc_mapped` (`cc.` presentation params, `perform.rs:1-11`), `recorded` (`UiState::recorder`).
  - Plugin only: `host_playing` (`lib.rs:277`), `host_clock` (`lib.rs:274`), `automation_target{lane}`: needs one small hook, the clap editor copies its lane targets into `UiState` each frame (it already reads them there, `crates/clap/src/lib.rs:646-660`).
- A missing target is reported with an Undo hint, never guessed (rule kept from `docs/signal-inspection/design.md:129-131`).

### 3.3 Runner
- `UiState::lesson: Option<lessons::Active { id, step, editor_instance, bindings, step_start }>` replaces `recipes: recipes::Learn` (`lib.rs:429`) and `first_run: Option<Tour>` (`lib.rs:194`). It runs where `recipes::frame` runs (`lib.rs:1040`) and draws where `tour::frame` draws (`lib.rs:1300`).
- A step advances when `done` holds (a short check mark so the user sees why) or on Next; Previous always works. Another document detaches it, as recipes do (`recipes.rs:376-381`), with "Reopen lesson patch".
- UI: the drawer's **Learn** tab (`drawer.rs:18-67`) becomes the hub: eight stage cards with progress (nothing locked), then the active step (the recipe panel layout, `recipes.rs:405-606`). A compact floating step card is used when the drawer is closed (Perform, the plugin's small window).
- View-only, as recipes are (`recipes.rs:5-8`): the runner never edits the patch and sends audio commands only from a button the user presses (test note, scene, Start).

### 3.4 Anchors and highlighting in egui
- Every widget records its rect under a stable key into `UiState::hits` each frame (`lib.rs:225-228`, `record` at `lib.rs:594`): `knob:4.cutoff_hz`, `module:4`, `in:5.cv`, `pcard:…`, `pexplain:…`, scene tabs, browser rows. The tour already spotlights by hits key (`tour.rs:199-201`). Lessons use the same keys with `{binding}` substitution and a trailing glob.
- Highlight = accent ring with a slow pulse painted in a Foreground `Area` (the tour's painting, `tour.rs:337-344`), **without the scrim and without capturing input** (the tour's scrim blocks the pointer, `tour.rs:345-364`; lessons must not). Anchor not on screen: the step offers "Show" (`explain::go`: pan, expand off-face control) or names the scene to open.
- Guard test: open each lesson patch headless through `kabl_ui::show` (as `crates/ui/tests/first_run.rs` does) and assert every step's anchor resolves after its `do` actions, so a renamed hit key fails CI, not a user.

### 3.5 Plugin
- Same `kabl_ui::show` (`crates/clap/src/lib.rs:776`), same runner and files. `where = "standalone"` steps are skipped with a one-line note; lesson 8 has plugin-only steps. The 1180×680 editor (`crates/clap/src/lib.rs:1005`) uses the floating step card.

### 3.6 Persistence
- `library::Prefs` (`library.rs:144-157`, saved in `library.json` under `KABL_USER_DIR` / `$XDG_DATA_HOME/kabl`, `library.rs:285-296`) gains `lessons: BTreeMap<String, Progress { step, done }>`; `first_run_done` (`library.rs:156`) becomes "lesson 00 dismissed". Shared by standalone and every plugin instance. Results are user sounds in the user library; nothing lesson-specific is saved in patches.

### 3.7 Localisation
- All lesson strings are in the TOML; later an overlay `lessons/<id>.<lang>.toml` with only `text`/`listen` fields. Nothing more now.

### 3.8 An earlier decision this reverses
- D03 said "No mandatory tutorial, unlocks, scores, runtime LLM or general curriculum engine" and fixed "exactly three" recipes in code (`docs/product-research/briefs/D03.md:96-98`). The pivot asks for one coherent learning system for eight stages; this engine keeps D03's other rules (optional, no unlocks, no scores, view-only, explicit targets).
## 4. Lesson-by-lesson outline

"Now" = buildable on existing modules, patches and UI. "Composites" = its final form needs the composite library (pivot C).

| # | Lesson | Patch | Steps (each is do + listen) | Completion checks | Result kept | When |
|---|---|---|---|---|---|---|
| 00 | Start here (first run) | Pluck line (`first_run.rs:48-69`); plugin: `default_patch()` | Section 2: play it, find a sound, change a macro, see how it works | `notes_played`, `opened_any`, `macro_moved`, `explain_open` | Nothing forced; offers Save As | Now (needs slice 2 for keys) |
| 1 | Find and play sounds | Library; one Keys and one Piece preset | Browse a shelf; preview two; open one; play it (keys / Start for a piece); favorite it; change one knob; Save As a variation | `preview_played`, `opened_any`, `notes_played` or `clock_running`, `favorited`, `param_moved`, `saved_as_user` | "My first variation" in your sounds | Now |
| 2 | Change sounds | A Voice preset with clear macros (e.g. Sock Puppet) + recipe *Pluck to pad* (`recipes.rs:77-138`) | Turn a macro; open its "?" and read the real targets; Show one target and turn it directly; compare with the reference (`compare.rs`); then pluck-to-pad envelope steps; pin a control to Perform | `macro_moved`, `explain_open`, `param_moved{target}`, `compare_restored`, pin presentation param added | Your pad, saved | Now |
| 3 | Signal flow | Pluck line or Reference (`patches/reference`), recipe *Filter movement* (`recipes.rs:139-210`) | Follow Output's cable back; inspect a tap at osc, filter, VCA and hear/see the difference; unplug the VCA cable and use "Why no sound?" (`inspect.rs:1-3`); plug back (Undo); turn on the LFO route | `inspecting{…}` ×3, cable removed then path restored (`reaches_output`), route bypass off | The moving-filter patch | Now |
| 4 | Build a voice | Init Keyboard (`browser.rs:35`, `factory:init-keyboard`) or an empty rack with MIDI In + Output | Add Osc, connect to Output, hear a drone; add VCA + ADSR, gate from MIDI; add a filter, envelope route to cutoff; tune the envelope | `has_module`, `connected{…}` for each cable, `reaches_output`, `notes_played`, `route{env→cutoff}` | Your own voice | Primitive version now. Final step "this is what the Voice composite contains; open it" needs composites |
| 5 | Movement | Recipe *Interlocking* (`patches/interlocking`, `recipes.rs:211-273`), plus a clock + seq + LFO teaching patch | Start the clock; a seq drives your voice from lesson 4; change length; add an LFO route; divider; swing/probability | `clock_running`, `connected{clock→seq}`, `param_moved{seq.length}`, `route{lfo→…}` | Your voice, sequenced | Core now on primitives; patches should switch to composite "clocked sequence"/"modulator" blocks once they exist |
| 6 | Perform | Performance / cue-macros presets (`patches/performance`, `patches/cue-macros`) | Perform view; macros; launch a cue (`cues.rs:1-9`); store a cue with macro targets and morph; MIDI-learn a knob (if a controller); record a take | `scene{perform}`, `cue_launched`, cue param written, `cc_mapped` (optional), `recorded` | A performance patch with your cue, and a WAV | Now |
| 7 | Reusable modules | Your lesson-4 voice, then a library composite | Select modules, encapsulate (`composites::encapsulate`, `composites.rs:155`); expose two controls and ports (`add_exposure`, :245); use it from the parent; publish (`publish`, :483); insert a library composite (`insert`, :347) and open its insides | `composite_exists`, `exposed{…}`, file published, composite inserted | Your voice as a reusable module | Mechanics now; content (a real library composite to open, good panels) needs composites. Build last |
| 8 | kabl in a DAW | Plugin: whatever is on the track; standalone: explanation + record | Plugin: play from the track's MIDI; host clock on; arm automation lane on a macro, write, play back; save project, reopen. Standalone: export a sound, open it in the plugin | `host_playing`, `host_clock`, `automation_target{lane}`, a macro moved by automation | A project with automated macros | Now on CLAP/REAPER; repeat on VST3/Ableton when pivot D lands |

The three D03 recipes stop being a separate feature: they become steps of lessons 2, 3 and 5.

## 5. Implementation backlog (bounded PRs, in order)

Every slice: own worktree, tests, a **real walkthrough capture** (xvfb video or numbered PNGs of the steps, through the actual app with a fresh `KABL_CONFIG_DIR`/`KABL_USER_DIR`), audio checks on `kabl_agents` only, independent review.

**L1. Lesson engine core, recipes migrated (M). Start now; independent of composites.**
- Scope: `crates/ui/src/lessons/{mod,run,panel}.rs`; TOML format and predicate enum (only those the three recipes need, plus `opened`, `param_moved`, `route`, `inspecting`, `compare_restored`); `lessons/` embedded by `build.rs`; progress in `library::Prefs`; Learn tab hub; delete `recipes.rs`, port its 3 recipes to `lessons/*.toml` with the same patches; port `tests/signal_inspection.rs:725` recipe checks.
- Acceptance: the three recipes behave as today (same patches, Show/Back/Inspect/Compare/test note); steps auto-advance on their checks; progress survives a restart and is shared with the plugin; anchor-resolution test passes; capture of one full recipe as a lesson. No change to patches or sound.
- Deps: none.

**L2. Play without a controller (M). Can run in parallel with L1 (different files).**
- Scope: computer-key notes (standalone: `app/` input into the notes ring; focus guard), on-screen key strip in `kabl_ui` (both builds), a UI note ring into the clap audio path; velocity fixed, octave shift; all-notes-off on focus loss.
- Acceptance: with no MIDI device, a keyboard preset sounds from A–K keys in standalone and from the strip in REAPER; no stuck notes on focus loss, scene change or sound load; typing in a text field never plays; existing MIDI unaffected. Capture + listening clip on `kabl_agents`.
- Deps: none.

**L3. First run on the engine (M).**
- Scope: lesson 00 as TOML; start in Rack with Pluck line playing; Start card (no scrim); anchor ring without input capture (`lessons/mark.rs`, from `tour.rs` painting); remove `tour.rs`; Sound-check strip with remembered output device and MIDI input (`style::Prefs`); plugin first-instance card; Pluck line macros if D7 = A (render-identical test); update `docs/ACCEPTANCE-V1.md` §1.
- Acceptance: fresh profile → sound within 2 s; every Start-card action works in any order; Esc / Not now never returns; nothing is blocked while the card shows; device + MIDI remembered; plugin shows the card once, silent until MIDI. Fresh-profile walkthrough capture, standalone and REAPER.
- Deps: L1, L2.

**L4. Macro reveal (S).**
- Scope: extend the existing card destination lines (`perform_view.rs:1508-1545`) with module number and signed depth while the macro is being turned; pulse the target knobs' rings in the rack when visible (`explain::destinations`).
- Acceptance: for 5 presets incl. Lichen and Acid Line (three ladders), every listed target names exactly one module and equals the real routes (unit test); capture of turning a macro. No patch change.
- Deps: none (can go any time; lesson 2 benefits).

**L5. Lessons 1, 2, 3 content (M).**
- Scope: three TOML lessons, teaching patches under `patches/lessons/` via test generators, remaining predicates (`favorited`, `saved_as_user`, `preview_played`, `reaches_output`, cable-removed).
- Acceptance: a newcomer-style walkthrough of each, end to end, on a fresh profile, ends with a saved user sound; listening prompts checked by ear (audio clips at each step for Kosta's listen). Kosta hands-on.
- Deps: L1, L3 (L2 for 1).

**L6. Lessons 6 and 8 (M).**
- Scope: Perform lesson; DAW lesson with plugin-only steps and the automation-lane hook in `crates/clap`.
- Acceptance: lesson 8 completes in REAPER (automation write/readback, project reopen); standalone shows its standalone path. Capture in REAPER on `kabl_agents`.
- Deps: L1.

**L7. Lessons 4 and 5, primitive version (M).**
- Acceptance: built from Init Keyboard, each cable step detects the user's own cable even when modules are added in another order (bindings by `added_in_step`). Capture.
- Deps: L1, L2.

**L8. Lesson 7 + composite tails of 4/5 (M–L).**
- Deps: composite library (pivot C) with at least a Voice and a clocked-sequence composite. Revisit lesson 4/5 patches then.

**L9. Walkthrough promo asset (S, later).** The pivot's E walkthrough is lesson 00 + 1–3 recorded; no separate script.

## 6. Owner decisions

**D1. First launch opens on the rack with the start patch playing, or on Sounds (today).**
- A: Rack, Pluck line already playing, Start card (proposed). B: Sounds open, nothing pushed (today, `startup.rs:236`). C: A silent rack until the user presses Start.
- Evidence: the patch sounds by itself (`clock.rs:136`), but on a first run under the Sounds page the user sees a browser and hears a sequence they cannot locate (see screenshots in section 1).
- Consequence: A gives sound in under 2 s and points at the instrument; it plays immediately, which some people dislike at launch (mitigated: Stop on the card).
- Recommend A.

**D2. Plugin: should the first instance show the Start card and lessons at all?**
- A: yes, once ever, shared progress (proposed). B: never in the plugin; lessons only in standalone. C: only when the user opens Learn.
- Evidence: the plugin uses the same UI (`crates/clap/src/lib.rs:776`); a DAW user cannot load the standalone if they only installed the VST3 on Windows.
- Recommend A (card is small and dismissible; plugin never makes sound by itself).

**D3. Preview when selected: on by default?**
- A: on for everyone. B: on only during lesson steps (proposed). C: off (today, `library.rs:153`).
- Evidence: "Browsing never makes sound" is an explicit rule (`browser.rs:9`); newcomers expect click-to-hear.
- Recommend B now; revisit A after a hands-on session.

**D4. Lessons in the binary (embedded TOML) or as editable files on disk / user-authored lessons.**
- A: embedded, factory only (proposed). B: also load `~/.local/share/kabl/lessons/*.toml` (community lessons). 
- Consequence: B is ~a day more and makes the format a public, versioned contract.
- Recommend A now; the format allows B later.

**D5. Lesson order and naming of the 8 stages.** Proposed change: lesson 00 "Start here" added; 4 and 5 built on primitives first, composites added later rather than waiting. Alternative: hold 4/5 until composites so they teach the composite form only. Recommend primitives first: building a voice from parts is what makes a composite understandable afterwards ("no fake abstraction").

**D6. Computer-keyboard layout and default on.** A: on by default in standalone, A-row (Ableton-like). B: off until enabled. Evidence: no controller = no way to play a keyboard sound today. Recommend A, with the key strip hidden until asked.

**D7. Give the start patch (Pluck line) macros.**
- A: add a `macro` module with two pinned macros (Brightness, Space) set so the sound is bit-identical at rest (proposed; verified by an offline render comparison with `first_run::render`, `first_run.rs:196`). B: keep it as is; lesson 00's "Change it" step loads a preset instead. C: pick a different start patch from the 100 presets.
- Evidence: Pluck line's Perform page is empty (`10-start-patch-perform.png`); every factory preset has macros. Changing a factory sound needs your OK (pivot process rule).
- Recommend A: the first change happens on the sound the user already hears.

Not decisions for Kosta (routine, the builder decides): file layout, predicate set, highlight style, test approach.
