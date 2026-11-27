# Lesson format

A lesson is one TOML file in `lessons/` at the repository root. `crates/ui/build.rs` embeds every
`lessons/*.toml` into the binary, so the standalone app and the plugin have the same lessons with
no install step. The Learn tab lists them in file-name order. The code is in
`crates/ui/src/lessons/`.

Lessons are optional and view only. A lesson never edits the patch and never locks anything. It
opens its start sound through the usual unsaved-changes question. It sends audio only when the
user presses one of its buttons.

## Example

```toml
id = "filter-movement"        # progress key in library.json; never rename it
title = "Filter movement"
summary = "One line on the Learn card."
start = { sound = "factory:recipes/filter-movement" }   # library id opened on start

[[step]]                      # one or more, shown in order
text = "What to do and what to listen for."
show = { route = { from = 7, port = "out", to = 3, kind = "filter.svf", param = "cutoff_hz" } }
inspect = { id = 7, kind = "lfo", port = "out" }
play_note = 4.0               # seconds, 0 < s <= 30
compare = true
anchor = "knob:3.cutoff_hz"
done = [{ route = { from = 7, port = "out", to = 3, kind = "filter.svf", param = "cutoff_hz" } }]

[end]
unguided = "A last task with no guidance."
```

## Lesson 00 and the Start card

`card = true` (lesson field, default false) marks the first-run lesson. It runs on the Start
card, docked at the bottom right of the page with no scrim, instead of in the Learn panel: the
rack stays usable, and the step's `anchor` ring shows without the drawer open. The card has
**Next**, **Skip** (Esc does the same) and **Stop**/**Play**, which stops or runs every clock in
the patch. If no audio output runs for two seconds it adds a "no sound output" line.

`start = { sound = "kabl:start" }` names the start patch (`crates/ui/src/first_run.rs`) instead
of a library sound. Starting such a lesson from Learn opens the start patch in place of a
sound with no unsaved changes, or asks the user to save first.

On a fresh profile the standalone app opens the start patch playing in the Rack with lesson 00
at step 1, and saves `first_run_done` at once, so an app closed mid-lesson does not start it
again. A profile with `first_run_done` (also what the old tour set) opens on Sounds as before.
The plugin never starts it. `lessons/00-start-here.toml` is lesson 00.

## Step fields

Only `text` is required. Each optional field changes what the step shows (a button, a ring, a strip or other text), in the order listed.

| field | button / effect |
|---|---|
| `title` | A short heading above the text (the Start card shows it). |
| `plugin_text` | Optional string. In a plugin the Start card and the Learn panel show it instead of `text` (the host's transport decides whether the sound plays, and the host owns the devices); both read it through `Step::text_for`. Absent: `text` is shown. Must not be empty or blank when present. |
| `include` | `{ lesson, step }`: this step is step `step` (counted from 1) of the lesson with that `id`, as it is there. See "Reusing a step". The step has no other field. |
| `show` | **Show** brings the target into view (and **Back** returns). Target is one of `{ control = { id, kind, param } }`, `{ route = { from, port, to, kind, param } }`, `{ module = { id, kind } }`. |
| `inspect` | **Inspect #id port** selects that output in the signal inspector. |
| `play_note` | **Play test note** plays middle C for this many seconds at a fixed velocity. |
| `compare` | **Open Compare**. |
| `sound_check` | Optional bool, default `false`. `true` shows the Sound-check strip on the Start card: output device, MIDI input and an output meter, remembered for the next launch. Standalone only: a plugin uses the host's devices. The parser does not reject it in a plugin lesson; it is only meaningful standalone. |
| `anchor` | A `UiState::hits` key (`knob:4.cutoff_hz`, `module:6`, `pcard:…`). While the step shows, it gets an accent ring with a slow pulse. The ring takes no input. A trailing `*` matches the first key with that prefix. |
| `done` | Completion conditions. Once all of them hold, the step advances by itself and the panel shows "Step n done." for a moment. If they already all hold when the step is reached, the step counts as done at once: the panel says so and **Next** moves on, nothing has to be undone and redone. A step completed earlier in the session stays done when **Previous** goes back to it. If `done` is empty, the step only advances on **Next**. **Next** and **Previous** always work: every step can be skipped. A later step whose conditions come to hold, compared with the patch as the lesson started, is marked done before it is reached. |

Targets are named by explicit module id and kind, never by name. If a target is missing or has
another kind, the step says so and suggests Undo. It never guesses.

## Conditions (closed vocabulary)

Each condition checks the real state. "Since the step started" means compared with the patch and
undo position saved when the step was entered.

| condition | holds when |
|---|---|
| `{ opened = "factory:…" }` | the open document is that library sound |
| `{ param_moved = { id, kind, param, by } }` | the parameter moved by at least `by` (in its own units) since the step started |
| `{ route = { from, port, to, kind, param } }` | that modulation route exists and is not bypassed |
| `{ route = { from, port, to, kind, param, amount_below } }` | as above, and its depth is below `amount_below` (-1 to 1) |
| `{ inspecting = { id, port } }` | the signal inspector has that output selected |
| `{ any_param_moved = { id, kind, params, by } }` | any one of `params` moved by at least `by` since the step started |
| `"sounds_open"` | the Sounds browser is open |
| `"compare_restored"` | the Compare reference was restored during this step and not undone |
| `"preview_played"` | a sound preview in Sounds is playing |
| `"notes_played"` | a note sounds: computer keys, the key strip or MIDI |
| `"favorited"` | the open document is a library sound that is in the favorites |
| `"saved_as_user"` | the open document is a user sound with nothing unsaved |
| `{ explain_open = { id, key } }` | Explain is open on control `key` of module `id` (from its Perform card's "?") |
| `{ pinned = { id, param } }` | control `param` of module `id` is pinned to Perform |
| `{ reaches_output = { id, kind } }` | module `id` (of `kind`) has a signal cable path to an output |
| `{ cable_removed = { from, port, to, input } }` | both modules are in the patch and no cable runs from `from`'s `port` to `to`'s `input` |

New conditions are new variants of `lessons::Cond`. They are not an expression language.

## Reusing a step

A step can point at a step of another lesson instead of repeating it. This is lesson 02
(`lessons/02-change-sounds.toml`), which takes steps 2 to 4 of the lesson with id `pluck-to-pad`
(`lessons/r1-pluck-to-pad.toml`):

```toml
[[step]]
include = { lesson = "pluck-to-pad", step = 2 }
```

`lessons::all()` replaces each `include` with a copy of the named step before anything else sees
the lesson, so the panel, the card and the runner never meet one. A lesson is rejected, left out of
the list, with an error in the log, when:

- `step` is 0 or past the last step of the named lesson;
- `lesson` is not the `id` of a lesson that loaded;
- the named step is itself an `include` (includes do not chain);
- the `include` step has any other field (the parser rejects it with "a step with `include` has
  no other field").

A lesson is 3 to 5 steps. `crates/ui/tests/signal_inspection.rs` checks that range on every
lesson, counting included steps; the parser does not.

## Following the lesson's own sound

Opening a different sound detaches an active lesson: its steps no longer apply. The exception is
a sound that one of the lesson's own `opened` conditions names. Opening that sound follows it: the
lesson stays attached and takes a fresh baseline from the new document.

## Steps already met

When a lesson starts, or follows into a sound, the runner records which steps' conditions already
hold (`Active::met_at_start`). Those steps are not marked done early: only conditions that come to
hold during the lesson count before the step is reached. So "plug the cable back" is not done by a
cable that was never unplugged.

## Validation

Unknown fields, unknown conditions, a missing `id`/`title`/`start`/`end`, no steps, empty `text`, an empty (or blank) `plugin_text`,
an empty `anchor`, an `include` step with any other field, a `play_note` out of range, a `param_moved.by` or `any_param_moved.by` that is not above 0, an empty `any_param_moved.params`, and a
`route.amount_below` that is not a number are all
rejected. The error names the file and the reason. The unit tests parse every embedded file, so
a bad file fails CI. `crates/ui/tests/signal_inspection.rs` also opens each lesson and checks
that every `show` target resolves and every `anchor` is drawn. `crates/ui/tests/lessons.rs`
walks every lesson but lesson 00 on its real patch (`crates/ui/tests/first_run.rs` walks lesson 00) and checks that each step completes only once its
conditions hold, and that the runner adds nothing to the op log.

## Progress

`library.json` (in `KABL_USER_DIR` or `$XDG_DATA_HOME/kabl`) has
`"lessons": { "<id>": { "step": n, "done": bool } }`, where `step` is the step the user was last
on and `done` means the end page was reached once. Reopening a lesson resumes at `step`; a
lesson left on its end page starts again from the first step. The standalone app and every plugin instance
share it. Files from before lessons existed have no `lessons` field and load with no progress.
