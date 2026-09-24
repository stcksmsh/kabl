# Composition + Motion: design decisions

Written before the launch code, as the brief asked: the pending-launch rules across Stop,
Restart and Load are fixed here first, and the tests in `crates/engine/tests/launch.rs` check
them. Rationale summaries also go into `docs/decisions.md`.

## 1. Pattern banks live in the sequencer's params

- Each `seq` has four banks, A–D. A bank holds, per step, pitch `p`, gate `g` (rest),
  velocity `v` and probability `r`, plus the bank's `length`, `gate_len` and `gate_mode`.
- **Bank A keeps the old param names** (`p1`…`p8`, `g1`…, `v1`…, `length`, `gate_len`,
  `gate_mode`); banks B–D use the same names with a prefix: `b.p1`, `c.length`, `d.v8`. New
  in A: `r1`…`r8` (probability, default 100 %).
  - So an old patch *is* bank A. Its routes, pins (`pin.p3`) and CC mappings (`cc.p3`) name
    exactly the params they named before. No migration step, no schema bump.
  - Every bank param is a real param: routes, pins and CC maps can target any bank, and the
    target's name says which (`c.p3` is bank C, step 3). The UI labels bank params with
    their bank everywhere outside the face (`C · P3`).
  - Rejected: a separate bank table in the patch (a second storage model, needs a new op and
    a migration, and routes could not target bank data); one param set + copying banks in and
    out on switch (editing an inactive bank would be impossible; a route's target would
    silently change meaning with the bank).
- Module-wide params: `transpose` (unchanged), `direction` (FWD/REV/PEND, a performance
  control like transpose) and `bank` (the **startup bank**, A by default).
- `MAX_PARAMS` rises to 143. The per-module stack array is initialized only up to the module's
  own count.
- **Edit bank is a view choice**, per sequencer, never saved and never undone (like
  expansion). It picks which bank the face, the advanced area and the drawer show. Changing it
  retargets nothing: routes, pins and mappings are stored by param name. Selecting a route or
  pin whose target is in another bank switches the edit bank to reveal it; it never launches.
- **Playing bank is runtime state** in the module, carried across graph swaps. A fresh graph
  (startup, Load) starts on the startup bank. Setting the startup bank is an explicit, undoable
  edit (`Set as startup`), and it does not change what is playing.
- **Bank ops** (one undo step each): copy bank X to Y (every bank param of Y set to X's values,
  including absent = default), clear (every step off, 0 st, 100 % velocity and probability;
  length 8; gate settings default), rename (a label `bank.<a..d>`, no audio rebuild). The
  face knobs edit the edit bank's params through the ordinary gesture path.

## 2. Step order, probability, reset

`length` = L (1–8), steps 1…L.

| Direction | Order | Endpoints |
|---|---|---|
| FWD | 1 2 … L 1 2 … | — |
| REV | L … 2 1 L … | — |
| PEND | 1 2 … L L-1 … 2 1 2 … (period 2L−2) | each endpoint plays once per turn |

- The start step is 1 for FWD and PEND (moving up) and L for REV.
- A direction change applies at the next advance, from the current step. PEND entered from
  FWD moves up, from REV moves down.
- A step beyond a shortened length: FWD goes to 1, REV to L, PEND turns down from L.
- **Reset input** (edge while the clock is high): jump to the start step now. Between pulses:
  the next clock edge plays the start step. The random stream is reseeded on every reset edge,
  so a Restart patched to reset replays the same probability decisions.
- **Probability** is decided once per step, on the edge that advances to it: one draw from the
  sequencer's own xorshift stream per advance, whatever the probabilities are (changing one
  never shifts the stream). A rejected step is a rest: pitch and velocity outputs still move,
  the gate stays low, the timing step is not skipped. `r` = 100 % always plays.
- The stream's seed is the module id (set at compile), so every fresh load of the same patch
  gives the same decisions; the stream state is carried through graph swaps, so edits never
  replay or skip decisions.

## 3. Launches

A launch asks one or more sequencers to switch bank at a boundary of a **reference clock**:

- **Now**: at once.
- **Next step**: the reference clock's next tick (16th).
- **Next bar** (default): the next tick whose count is a multiple of 16. The count is kept by
  the clock: its first pulse after a load is tick 0, and a Restart's pulse is tick 0 again.

At the boundary each target sequencer is armed; it switches on the **first rising edge of its
own clock input at or after the boundary sample**, and that edge plays the new bank's start
step. Divider phase is untouched: no edge is made up, so a /2 sequencer whose next edge is a
tick after the boundary starts one tick later than a /1 sequencer. Both run from the same
boundary.

- The boundary is found on the audio thread, sample-accurately: the clock records the sample
  offset of each tick it starts in the block, and the engine arms the sequencer with that
  offset before the sequencer runs (clocks are scheduled first).
- **Replacing:** one pending launch per sequencer. A new launch for it replaces the old one
  (queued or armed); nothing builds up.
- **Cancel:** removes the pending launch (and the arm) of one sequencer, or all.

### Across transport, swaps and Load

| Situation | Behaviour |
|---|---|
| Launch while the reference clock is stopped | Selection: the sequencer is armed at once and waits for its next clock edge, which plays the new bank's start step. No note, no transport change. |
| Stop with a launch pending on that clock | The launch becomes a selection (armed now). Run begins the selected patterns on their first edges. |
| Restart (running) with a launch pending | The Restart pulse is tick 0, a bar and step boundary: the launch lands on it. |
| Restart while stopped | Nothing until Run; then as above. |
| Graph swap (any edit, undo, redo) | Pending launches live in the engine, arms and playing banks in the carried sequencer state: nothing is lost, nothing replays. Undo of an edit never re-sends a launch (launches are not ops). |
| Load | Pending launches are dropped; the loaded sequencers start on their startup banks, unarmed. |
| Reference clock deleted | Its pending launches are dropped when a graph without it plays; arms already made stay. |
| Target sequencer deleted | Nothing to switch; the entry drops at its boundary. |

## 4. Cues (Stage B)

A `cues` module holds up to eight cues as presentation params (`cue<N>.*`) and labels
(names): a reference clock id, a timing, and a bank (or "keep") per sequencer. Launching a
cue sends one launch with every target. Definitions are saved, undoable edits; a launch is a
runtime command. Deleting a module removes the cue entries naming it in the same undo step,
so undo restores them with the module.
