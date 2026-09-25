# D03 inspector cost (cloud VM, 4 vCPU, no RT priority)

Same host, same settings, runs interleaved. 48 kHz, 256-frame callbacks (budget 5333 µs).
Not laptop evidence and not an overhead-target pass: no target was set, and VM scheduling
noise dominates the tails.

## Offline, engine only (`probe_cost`, release build)

`cargo run --release -p kabl-engine --example probe_cost -- <patch> 60 3`: the same patch
and scripted chords rendered as 256-frame callbacks (four 64-sample blocks), 60 s × 3 runs
per mode, modes interleaved. Timed: swap install, commands, four blocks, report hand-off.
No audio device, so no arrival lateness or xruns exist here. The tap is on a per-voice
filter output (8 lanes), the dearest case in each patch. Raw:
[`probe-cost-init-keyboard.txt`](probe-cost-init-keyboard.txt),
[`probe-cost-composition.txt`](probe-cost-composition.txt).

Simple voice (Init Keyboard, 6 modules), 33 750 callbacks per mode:

| mode | p50 µs | p90 | p99 | p99.9 | max | mean |
|---|---|---|---|---|---|---|
| off | 107.8 | 130.8 | 210.0 | 360.4 | 2014.8 | 115.1 |
| on | 116.9 | 143.4 | 226.7 | 591.0 | 2294.5 | 126.3 |
| off + swap every 20 callbacks | 107.8 | 130.8 | 219.4 | 429.0 | 1925.5 | 115.8 |
| on + new selection every 10 + swap every 20 | 117.3 | 227.9 | 384.2 | 674.7 | 3795.4 | 147.6 |

Dense piece (Composition, 39 modules), 33 750 callbacks per mode:

| mode | p50 µs | p90 | p99 | p99.9 | max | mean |
|---|---|---|---|---|---|---|
| off | 276.8 | 316.4 | 492.6 | 883.4 | 59128.9 | 294.2 |
| on | 287.2 | 328.0 | 530.9 | 1002.2 | 4869.3 | 303.6 |
| off + swaps | 276.6 | 328.9 | 536.0 | 1223.9 | 11449.6 | 298.2 |
| on + selection + swaps | 285.8 | 574.4 | 842.1 | 1311.7 | 4752.5 | 351.5 |

Reading: a steady tap on an 8-lane output adds about 9–11 µs at the median (≈0.2 % of the
budget) and 3–5 % of mean callback time. Changing the selection every 10 callbacks moves the
p90 up (each change resolves the target by a linear pass over the graph's instances and
restarts the window); that rate (≈19 selections per second) is far above what a person
does. The single 59 ms maximum in the dense "off" run is a VM stall with the tap off.

Memory: 720 B of tap state inline in every compiled graph (allocated with the graph, off the
audio thread); a 552 B report; the report queue holds 8 (4.4 KiB). The engine keeps one
pending report (552 B).

## Real app (`scripts/perf.sh`, release build, Composition)

Each run: kabl-ui on `patches/composition` (running), PipeWire null sink, 48 kHz / 256
frames, no MIDI. *off*: inspector closed, 62 s. *on*: inspector open on Filter #26 lp for
60 s. *busy*: inspector open, alternating selections (lp/bp) with undo/redo graph rebuilds
about 2.5 times a second (≈74 s, so more callbacks). Values from each run's
`KABL_STATS_FILE` ([`perf-app-stats.txt`](perf-app-stats.txt)); "late" = execution longer
than the callback's audio; arrival = interval between callback starts (late when over
1.5 × the period); xruns = reported by the backend. Execution quantiles are upper edges of
50 µs bins; the last bin is open (> 3150 µs).

| run | callbacks | worst run µs | over half | late runs | worst arrival µs | late arrivals | xruns | p50 | p99 | p99.9 |
|---|---|---|---|---|---|---|---|---|---|---|
| off-1 | 12186 | 13432 | 37 | 12 | 24850 | 726 | 11 | ≤400 | ≤900 | >3150 |
| off-2 | 12240 | 7985 | 15 | 5 | 12633 | 395 | 0 | ≤400 | ≤750 | >3150 |
| off-3 | 12167 | 7733 | 20 | 3 | 22848 | 474 | 2 | ≤400 | ≤800 | >3150 |
| on-1 | 12392 | 7939 | 26 | 3 | 34201 | 559 | 4 | ≤400 | ≤850 | >3150 |
| on-2 | 12539 | 5670 | 10 | 2 | 17880 | 388 | 1 | ≤400 | ≤750 | ≤1900 |
| on-3 | 12572 | 7993 | 22 | 6 | 17542 | 647 | 1 | ≤400 | ≤800 | >3150 |
| busy-1 | 14155 | 14728 | 20 | 8 | 19166 | 739 | 3 | ≤400 | ≤800 | >3150 |
| busy-2 | 14171 | 10918 | 37 | 7 | 41643 | 650 | 4 | ≤400 | ≤850 | >3150 |
| busy-3 | 14353 | 9524 | 20 | 5 | 29943 | 546 | 1 | ≤400 | ≤800 | >3150 |

(The stats files were written by the build before the open last bin was labelled; their
"≤3200" is the open bin, shown here as "> 3150".)

Reading: on this VM the run-to-run spread (off: 0–11 xruns, 3–12 late runs) is larger than
any difference between modes. The medians and p99 bins are the same with the inspector off,
on and busy. This does not show the inspector causes or prevents xruns here, and says
nothing about the laptop.

## D03-R1 re-measurement on the final product head (99c28d3)

Everything above this section is **historical**. It was measured at `ef9ed7b`–`db29445`,
before the review fixes F1–F10 and N1–N3 and before the R1 error-ownership change, so it
does not measure the changed implementation. The numbers below come from the release build
of `99c28d3`. That commit's product code equals `25a9f78` plus the extended `probe_cost`
example; later commits change comments and docs only. Same VM class, 48 kHz, 256-frame
callbacks (5333 µs budget), no RT priority, no overhead target.

### Offline, engine only (`probe_cost`, 60 s × 3 runs per mode, interleaved)

`probe_cost` now also has **edits+topo** modes. Every 20 callbacks a graph with one
parameter nudged ±1 % is swapped in. Every 10th swap removes one signal cable, and the swap
after that restores it: that is the topology change, and the measurement window restarts
there (N1). Raw output: [`../r1/probe-cost-init-keyboard.txt`](../r1/probe-cost-init-keyboard.txt),
[`../r1/probe-cost-composition.txt`](../r1/probe-cost-composition.txt).

Simple voice (Init Keyboard, 6 modules; tap on SVF #3 lp, 8 lanes), 33 750 callbacks per mode:

| mode | p50 µs | p90 | p99 | p99.9 | max | mean |
|---|---|---|---|---|---|---|
| off | 82.0 | 92.6 | 115.5 | 225.0 | 1923.5 | 82.6 |
| on | 87.1 | 102.4 | 144.6 | 194.2 | 1036.8 | 89.8 |
| off + swaps | 81.1 | 105.9 | 143.9 | 658.0 | 3109.4 | 84.5 |
| on + selection + swaps | 89.2 | 154.0 | 215.3 | 306.3 | 1416.6 | 99.5 |
| off + edits + topology | 79.9 | 142.4 | 192.0 | 312.1 | 2411.8 | 85.5 |
| on + edits + topology | 85.5 | 147.8 | 200.9 | 555.1 | 4183.4 | 93.0 |

Dense piece (Composition, 39 modules; tap on SVF #26 lp), 33 750 callbacks per mode:

| mode | p50 µs | p90 | p99 | p99.9 | max | mean |
|---|---|---|---|---|---|---|
| off | 211.9 | 248.2 | 415.1 | 887.6 | 3320.8 | 220.3 |
| on | 223.1 | 249.9 | 387.1 | 1605.9 | 3532.5 | 228.6 |
| off + swaps | 215.2 | 243.9 | 361.1 | 1564.7 | 5471.5 | 222.7 |
| on + selection + swaps | 216.3 | 431.4 | 674.8 | 926.3 | 6638.5 | 256.6 |
| off + edits + topology | 216.3 | 436.2 | 707.5 | 1640.0 | 4016.2 | 265.3 |
| on + edits + topology | 226.2 | 446.9 | 699.4 | 925.0 | 4710.4 | 266.6 |

**Reading.**

- A steady tap costs about 5–11 µs at the median, as before. The absolute times are lower
  than the historical table because this is a different VM instance; compare modes only
  within one table.
- Continuing the measurement window through parameter swaps, and restarting it on a
  topology change, costs about the same as the edits alone. For the dense piece, "on"
  versus "off" with edits and topology is 266.6 versus 265.3 µs mean and 446.9 versus
  436.2 µs p90. The higher p90 in both edit modes comes from the swaps themselves: the
  crossfade renders two graphs.
- The tails (p99.9 and max) swing in both directions between modes and runs. That is VM
  noise.

**Sizes.**

- Tap state: 720 B per graph (unchanged).
- Report: **560 B**, up from 552 B, because F4 added `end_sample`.
- Report queue: 8 × 560 B.
- Stream-error queue: 16 slots of `cpal::Error`, down from 256. At most 16 errors are
  outstanding (R1).

### Real app, dense piece (`scripts/perf.sh`, RUNS=3, plus one extra busy run)

Composition, release build of `99c28d3`, 48 kHz/256, run through the PipeWire null sink,
xdotool on Xvfb. The modes:

- **off**: inspector closed.
- **on**: inspector open on Filter #26 lp for 60 s.
- **busy**: the selection moves between outputs while undo/redo rebuild the graph about
  2.5 times a second.

Runs were interleaved. Raw data: [`../r1/perf-app-summary.txt`](../r1/perf-app-summary.txt).
Callback **execution**, **arrival** lateness and **xruns** are kept apart. The p-columns are
histogram bins (50 µs) after the first second.

| run | callbacks | exec p50 | p99 | p99.9 | exec worst µs | > half budget | late executions | late arrivals (worst µs) | xruns |
|---|---|---|---|---|---|---|---|---|---|
| off-1 | 12098 | ≤300 | ≤500 | >3150 | 5851 | 23 | 6 | 758 (19373) | 6 |
| off-2 | 12097 | ≤300 | ≤550 | ≤3150 | 17147 | 12 | 4 | 547 (24756) | 2 |
| off-3 | 12105 | ≤300 | ≤500 | ≤3100 | 11642 | 19 | 3 | 583 (29329) | 2 |
| on-1 | 12500 | ≤300 | ≤550 | >3150 | 6081 | 16 | 7 | 821 (18489) | 7 |
| on-2 | 12522 | ≤300 | ≤500 | >3150 | 15094 | 16 | 8 | 617 (21178) | 3 |
| on-3 | 12517 | ≤300 | ≤500 | ≤2700 | 5429 | 13 | 2 | 611 (49192) | 0 |
| busy-1 | 4277 | ≤300 | ≤550 | ≤2800 | 5127 | 5 | 0 | 164 (21272) | 1 |
| busy-2 | 14014 | ≤300 | ≤500 | >3150 | 5598 | 22 | 4 | 773 (17234) | 0 |
| busy-3 | 14059 | ≤300 | ≤550 | >3150 | 5569 | 28 | 5 | 643 (48755) | 0 |
| busy-4 | 13988 | ≤300 | ≤500 | >3150 | 20044 | 18 | 6 | 797 (22923) | 2 |

**`busy-1` stopped early.** At 24.4 s the audio stream **stalled**: no callbacks for over
1.5 s after callback 4277, which the app logged at WARN
([`../r1/perf-busy-1-stall-kabl.log`](../r1/perf-busy-1-stall-kabl.log)). The UI kept
running, but the stream did not recover. This is the cloud stream stall already reported in
earlier batches, and recovery belongs to D05. I can't tell from one occurrence whether the
inspector's busy mode made it more likely. `busy-4` was run to give three complete busy
runs; `busy-1` stays in the table as observed. The same stall also hit the separate logging "blocked" run in
this session (`r1/logging/blocked.txt`: "stalled … after 2428").

**Reading.** The p50 and p99 bins are the same in every mode. The worst execution times,
late executions and xruns vary more from run to run within a mode than between modes, as in
the historical runs. These are VM measurements, not laptop results.
