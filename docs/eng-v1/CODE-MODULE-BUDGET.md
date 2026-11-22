# Code-module time budget

The policy for how much CPU time a code module (user WASM, D11/D12) may use per block. It follows
from the REAPER benchmark in PR #181 ([REAPER-CODE-BENCH.md](REAPER-CODE-BENCH.md)). That benchmark
found that only two `demo.bitfilter` modules fit at 8 voices. It put the blame on turning time into
fuel at the worst calibration probe. Looking closer, four things were wrong. The first was the most
damaging:

1. **An out-of-fuel stop killed SDK modules for good.** The SDK guards each export with a busy
   flag. When the host stops a call part-way, the flag stays taken, and every later call trapped
   on it. One late block therefore crashed the module permanently. This is why the third filter
   in #181 was silent from the start.
2. **Every copy of one module shared one share.** All places of the same blob share one `Health`,
   so six copies of a filter split a single 25 % share six ways while the 50 % total sat unused.
3. **Fuel was converted at the worst probe**, which gives typical DSP about a third of its time.
4. **The wall-clock backstop was sliced per voice** (10 µs for 32 voices), which is within the
   normal jitter of a 5 µs call. Eight jittery calls in a row were a crash.

Code: `crates/modules/src/code/budget.rs` (shares, calibration), `module.rs` (`Health`: fuel range,
measured rate, placements, overruns, suspension), `scope.rs` (notes to the user),
`sdk/kabl-module-sdk/src/rt.rs` (busy flag).

## Policy

1. **Time shares.** The block period is the engine block: 64 frames, 1333 µs at 48 kHz.
   - Each place of a code module in a patch may use 25 % of it, and all code modules together
     50 %. When the requests add up to more than 50 %, every share is scaled down in proportion.
   - Only modules with live voices count. A share covers all of its voices.
   - At least half of every block is left for the built-in engine. The perf gate (`bench_patches`)
     holds built-in patches to a fraction of the same 1333 µs.
2. **Fuel range.** A share of `t` µs is turned into a fuel range:
   - the floor is `t` × the slowest calibration probe's rate. This is the old, always-safe budget.
   - the ceiling is `t` × the fastest probe's rate, and never less than 4000 fuel/µs.
3. **Measured rate.** Every call is timed. Fuel and time are summed until they add up to 100 µs
   of run time. The rate of that sum moves the module's budget within the range: half the way
   down, a quarter of the way up. A new module starts at the ceiling. The audio thread only
   touches atomics.
4. **Enforcement (real-time safe).**
   - A call that runs out of fuel makes its block silent, with a 5 ms fade.
   - A call that takes the whole share of wall-clock time on its own is counted as an overrun.
   - Eight overruns in a row make that voice sit out 512 blocks (about 0.7 s): it is silent and
     not called, then it tries again.
   - An overrun is never a crash. Traps, NaN output and memory growth still are.
5. **Hard cap.** The ceiling is a fixed amount of fuel, so every call ends. A runaway module (code
   that is slow per unit of fuel) can run for at most its share × ceiling/floor before fuel stops
   it. That is about 3.8× its share on the i7-13700H, for one rate window, before its measured
   rate brings it to the floor. An endless loop runs 8 blocks in every 520, so it costs about 1.5 %
   of its share on average. Nothing can stall the audio thread.
6. **The user is told.** The control thread checks every running module on each UI tick.
   - The first overrun gives a toast: "custom module X went over its CPU time budget in N
     block(s)". It comes again each time the count doubles.
   - A crash gives "custom module X stopped: …".
   - Before this change, neither reached the UI.
7. **SDK.** On wasm, a taken busy flag can only mean such a stop, so the SDK carries on. The
   example blobs are rebuilt, and their golden renders are unchanged. Modules built with an older
   SDK keep the old behaviour until they are rebuilt.

## Numbers

i7-13700H, REAPER 7.75, 48 kHz, device block 128. The patch is `osc.va → demo.bitfilter × N → vca`,
with an 8-note chord held for 30 s and rendered offline. Calibration on an idle machine: slowest
probe 1261 fuel/µs (`spin`), fastest 4893 (`mem`).

| N filters | before (#181) | after: render of 30 s | silent blocks |
|---|---|---|---|
| none | 0.92 s | 0.90 s | 0 |
| 2 | 3.11 s | 2.44 s | 0 |
| 3 | silent (crash) | 3.36 s | 0 |
| 4 | – | 4.38 s | 0 |
| 6 | – | 6.32 s | 0 |
| 8 | – | 8.59 s (see the open issue below) | 0 |

- One filter at 8 voices costs about 0.91 s per 30 s of audio: about 3 % of one core, or about
  40 µs per 64-frame block (about 5 µs per voice call, 11.8k fuel).
- At 8 voices, 6 filters take about 243 µs of the 1333 µs block (18 %), and 8 filters about
  325 µs (24 %). The whole plug-in with 8 filters uses about 29 % of the block.
- The code-module total is capped at 50 %. A 2021–2023 laptop core at half this speed still fits
  8 filters inside that cap.
- In the plug-in, the measured rate settled at about 2000 to 3000 fuel/µs. The budget per voice
  call was 20k to 30k fuel against the 11.8k needed.

**Open issue (not the budget).** At 8 or more filters, most REAPER runs render silence in about
0.4 s, less than the baseline. One 8-filter run rendered fully (the 8.59 s above). Instrumented
runs show the modules running with healthy budgets (no overruns that matter) while REAPER plays
before the render. During the offline render itself, the code modules are not called at all. Some
plug-in or host path stops processing the voices for large code patches. That is separate from
this policy and needs its own investigation.

## Trade-offs

- A module that is slow per unit of fuel (call-heavy or divide-heavy code) can overshoot its share
  for one rate window before its rate is measured. The bound is item 5 above. The old policy never
  overshot, but it gave typical DSP about a third of its time.
- On a busy machine the measured rates go down (the calls get preempted), so modules get less fuel,
  and some blocks are muted. With suspension instead of a crash, they come back when the load
  drops.
- The calibration probes run at idle priority. On a loaded machine they report nonsense (`call`
  at 3 fuel/µs under a load of 80). The 4000 fuel/µs minimum ceiling and the per-module
  measurement make the budget independent of that.
- An endless loop is no longer disabled for good. It is muted and suspended over and over, at
  about 1.5 % of its share, and the toast keeps the user told.
- Several songs in one plug-in host share the process-wide module registry. An overrun toast
  appears in whichever instance's UI checks first, and copies of one blob across songs add their
  places together.
