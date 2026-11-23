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

## Step fields

Only `text` is required. Each optional field adds one button to the step, shown in this order.

| field | button / effect |
|---|---|
| `show` | **Show** brings the target into view (and **Back** returns). Target is one of `{ control = { id, kind, param } }`, `{ route = { from, port, to, kind, param } }`, `{ module = { id, kind } }`. |
| `inspect` | **Inspect #id port** selects that output in the signal inspector. |
| `play_note` | **Play test note** plays middle C for this many seconds at a fixed velocity. |
| `compare` | **Open Compare**. |
| `anchor` | A `UiState::hits` key (`knob:4.cutoff_hz`, `module:6`, `pcard:…`). While the step shows, it gets an accent ring with a slow pulse. The ring takes no input. A trailing `*` matches the first key with that prefix. |
| `done` | Completion conditions. Once all of them hold, the step advances by itself and the panel shows "Step n done." for a moment. If `done` is empty, the step only advances on **Next**. **Previous** always works. |

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
| `{ inspecting = { id, port } }` | the signal inspector has that output selected |
| `"compare_restored"` | the Compare reference was restored during this step and not undone |

New conditions are new variants of `lessons::Cond`. They are not an expression language.

## Validation

Unknown fields, unknown conditions, a missing `id`/`title`/`start`/`end`, no steps, empty `text`,
an empty `anchor`, a `play_note` out of range and a `param_moved.by` that is not above 0 are all
rejected. The error names the file and the reason. The unit tests parse every embedded file, so
a bad file fails CI. `crates/ui/tests/signal_inspection.rs` also opens each lesson and checks
that every `show` target resolves and every `anchor` is drawn.

## Progress

`library.json` (in `KABL_USER_DIR` or `$XDG_DATA_HOME/kabl`) has
`"lessons": { "<id>": { "step": n, "done": bool } }`, where `step` is the furthest step reached
and `done` means the end page was reached once. The standalone app and every plugin instance
share it. Files from before lessons existed have no `lessons` field and load with no progress.
