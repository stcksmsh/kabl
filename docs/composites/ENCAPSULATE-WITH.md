# Curated encapsulation for generators

Gap G5 in `docs/pivot/composite-catalogue.md`, roadmap row CI2. The factory patches are built
by `tools/examples/presets_*.rs`. To turn part of a generated patch into a composite, a
generator needs one call that wraps chosen leaf modules with a curated public interface and
writes the same bytes every time.

## The API

```rust
kabl_ui::composites::encapsulate_with(
    log: &mut kabl_core::PatchLog,
    name: &str,
    definition: kabl_core::Definition,
    members: BTreeSet<ModuleId>,
    ports: &[kabl_core::Exposure],      // { label, target }
    controls: &[kabl_core::Exposure],
    panel: Option<kabl_core::panel::Panel>,
) -> Result<CompositeId, String>
```

- `members` are leaf modules that no composite owns yet. The new composite is top level.
- `definition` is the caller's. A composite that stands for a factory definition uses its
  factory id and version, `kabl.<name>` vN (see `FACTORY-BANK.md`).
- `ports` then `controls` become interface keys `1..=n` in the order given, so a `panel` can
  place them by key. A port binds a module port or a parameter (a route into it); a control
  binds a parameter.
- The interface is exactly what the caller lists; nothing is added. Every cable that crosses
  the boundary must have its inside end listed as a port, and every pinned parameter inside
  the set must be listed as a control (interactive encapsulation keeps pins as controls, so a
  curated interface must say what to call them).
- The candidate patch is validated and compiled before anything is written.

The `mapping` part of an exposure (display range, unit, curve, scaled multi-target) is gap G2,
roadmap row CI5. `Exposure` has no such field yet, and this API adds none. Cable-parameter
bindings are the one schema change; see below.

## Refusals

Each returns an error and leaves the log untouched:

| Case | Error names |
|---|---|
| An exposure target outside `members` (a cable parameter's owner is outside) | the exposure's label and target |
| A control whose target is not a module parameter (cable parameters are ports only) | the label and target |
| Two exposures with the same label | the label |
| The same target listed twice as a port, or twice as a control | the label and target |
| A boundary crossing no port covers | every such cable, with its inside and outside ends (`#<module>.<port>`) |
| A pinned parameter no control covers | the pin (`#<module>.<param>`) |
| One outside source feeding two inside targets (fan-in, G3, CI4) | both cables and the source |
| A member that is missing or already owned | the module |

Interactive encapsulation still accepts fan-in by giving each target its own port. The curated
call refuses it because a curated interface would need one public input for several targets,
which `Exposure` cannot hold until CI4.

## Cable parameters (G1, CI3)

A route can move a parameter of another cable (`PortRef::CableParam`): most often a macro
driving the amount of an envelope → cutoff route. Such a parameter belongs to the module its
cable ends on (`PatchState::end_module`, `kabl_core::composite::owner`). The route into it
crosses the boundary when its source and that owner sit on different sides:

| Source | Owner (where the moved cable ends) | Result |
|---|---|---|
| outside | inside | the route crosses; its inside end is the cable parameter, which becomes a port (interactive) or must be listed as one (curated) |
| inside | outside | the route crosses; its inside end is the source's output jack |
| same side | same side | no crossing, whichever way the moved cable itself runs |

So when the moved cable itself straddles the boundary, only its destination decides: an
envelope inside driving a cutoff outside leaves the amount outside, and the reverse brings it
in. The flat graph does not change; the outside route still targets the same cable, so cues,
CC maps on the macro and runtime writes to the route (`RuntimeTarget::Route`) resolve as
before and the render is bit-identical. Opening the composite shows the inner route and its
amount, editable as usual.

A cable parameter can be a port, not a control: composite faces and panels only draw knobs
for module parameters, so the core format refuses a cable-parameter control too
(`Composite <id>: control must bind a parameter`).

The binding follows its cable. Removing the owner or the cable's source module, disconnecting
the cable, or reconnecting the cable's id so it ends outside the composite drops the port in
the same entry, and undoing that entry puts the port back.

Duplicating or publishing a composite copies the inner cables with new ids and rebinds the
ports to them. A port on the parameter of a cable that crosses the boundary is dropped from
the copy, because that cable is not copied; the original keeps it. A `.kablc` package is
schema 2 when it still binds a cable parameter and schema 1 otherwise, so an older build
refuses a schema 2 package as unsupported.

**Schema.** Binding a cable parameter is schema v9. A v8 build (including `v1.0.0-rc.2`)
refuses such a file on load with `Composite <id>: invalid public binding <key>`, so the
writer chooses the version per file: v9 only when a saved log entry binds a cable parameter,
v8 otherwise.
Patches without one, every factory patch included, keep their bytes. Both versions load.

## Stability guarantees

- The call appends exactly one entry, `Op::SetComposite`, after the existing ones. No earlier
  entry, module, cable, module id, cable id or id order changes. The flat leaf graph is the
  same, so the sound is bit-identical, and everything that addresses leaf ids keeps working:
  macro routes, cues, MIDI CC maps (`cc.<param>`) and host automation lanes. Pins keep
  working because each must be a curated control.
- The composite id is the next id the log has never used, counting composites in the state and
  every `SetComposite` in the log, undone ones included.
- The entry's timestamp is the previous entry's (`0` for an empty log). Nothing in the entry
  depends on the clock, the process or the run, so the same log and arguments always append
  the same bytes. A generator's whole file is stable only if its earlier entries are; the
  `PatchEditor` stamps them with the wall clock.
- A migration (CM1) must append to the existing generated log. Rebuilding the patch instead
  would renumber cables and change the sound.

Tests: `crates/ui/tests/composites_encapsulate_with.rs`.

## Generator example

```rust
use kabl_core::{Definition, Exposure, PortRef};
use std::collections::BTreeSet;

let mut log = rack.e.log().clone();
let jack = |id, port: &str| PortRef::Module { id, port: port.into() };
kabl_ui::composites::encapsulate_with(
    &mut log,
    "Space",
    Definition { id: "kabl.space".into(), version: 1 },
    BTreeSet::from([echo, room, mix]),
    // Ports: one per boundary crossing (here the send in and the return out).
    &[
        Exposure { label: "In".into(), target: jack(echo, "in") },
        Exposure { label: "Out".into(), target: jack(mix, "out") },
    ],
    &[Exposure {
        label: "Echo".into(),
        target: PortRef::Param { id: echo, param: "mix".into() },
    }],
    None,
)
.expect("Space wraps");
kabl_core::save(&dir, &log).expect("save");
```
