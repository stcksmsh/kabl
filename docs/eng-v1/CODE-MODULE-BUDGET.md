# Code-module time budget

The policy for how much CPU time a code module (user WASM, D11/D12) may use per block. It follows
from the REAPER benchmark in PR #181 ([REAPER-CODE-BENCH.md](REAPER-CODE-BENCH.md)). That benchmark
found that only two `demo.bitfilter` modules fit at 8 voices. The time shares were fine; the
problem was turning time into fuel at the worst calibration probe.

Code: `crates/modules/src/code/budget.rs` (shares, calibration), `module.rs` (`Health`: fuel range,
measured rate, overrun count), `scope.rs` (notes to the user).

## Policy

1. **Time shares (unchanged).** The block period is the engine block: 64 frames, 1333 µs at
   48 kHz. Each code module may use up to 25 % of it, and all code modules together up to 50 %. When
   the requests add up to more than 50 %, every share is scaled down in proportion. Only modules
   with live voices count toward the total. A module's share covers all of its voices together.
   This leaves at least half of every block for the built-in engine. The perf gate (`bench_patches`)
   holds built-in patches to a fraction of the same 1333 µs.
2. **Fuel range per module.** A share of `t` µs is turned into a fuel range:
   - the floor is `t` × the slowest calibration probe's rate. This is the old budget, which is
     always safe.
   - the ceiling is `t` × the fastest probe's rate, and never less than 4000 fuel/µs.
3. **Measured rate.** Every block call is timed already, for the wall-clock backstop. The fuel it
   burned in that time gives the rate this module runs at. The module's budget then moves inside
   the range to the fuel that fits its share at that rate. It moves down by a quarter of the gap
   on each call and up by a sixteenth, so a slow module is reined in within a few calls, and one
   preempted call costs little. A new module starts at the ceiling. The audio thread only touches
   atomics.
4. **Enforcement (unchanged, real-time safe).** A call that runs out of fuel makes that block
   silent, with a 5 ms fade. A call that runs past its share of wall-clock time is counted as an
   overrun. Eight overruns in a row are a crash: every voice fades out and stays silent until the
   user reloads the module.
5. **Hard cap.** The ceiling is a fixed amount of fuel, so a call always ends. A runaway module
   (one whose code is slow per unit of fuel) can run for at most its share × ceiling/floor, about
   4.6× its share on the i7-13700H, before fuel stops it. That is one block, or a few for a single
   voice. After that its measured rate brings it to the floor. One module (≤ 25 %) can therefore
   cost at most about 1.15 block periods once, never a stall.
6. **The user is told.** The control thread checks every running module on each UI tick. The first
   overrun gives a toast: "custom module X went over its CPU time budget in N block(s)". The toast
   comes again each time the count doubles, so a struggling module does not post one per block. A
   crash gives "custom module X stopped: …". Before this change, neither one reached the UI.

## Numbers

FILLED_BELOW

## Trade-offs

- A module that is slow per unit of fuel (call-heavy or divide-heavy code) can overshoot its share
  for a block or two before its rate is measured. The bound is item 5 above. The old policy never
  overshot, but it gave typical DSP about a quarter of its time.
- On a busy machine the measured rates go down (the calls get preempted), so modules get less fuel
  and may go silent. This is honest for real-time playback, but an offline render on a loaded
  machine can mute modules that would fit on an idle one.
- The calibration probes run at idle priority. On a loaded machine they report nonsense (`call`
  at 3 fuel/µs under a load of 80). The 4000 fuel/µs minimum ceiling and the per-module
  measurement make the budget independent of that. A garbage floor only matters for modules that
  really are that slow.
- Several songs in one plug-in host share the process-wide module registry, so an overrun toast
  appears in whichever instance's UI checks first.
