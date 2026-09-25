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
