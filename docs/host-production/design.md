# D08 design and contracts

## Stable automation

The bank is fixed at 16. Native `output_gain` remains first and unchanged; slot identities are `slot_1` through `slot_16`, followed by `host_clock`. REAPER native indices are gain0, slots1–16, Host17 (Bypass/Wet/Delta are host additions). Target identity is module ID + kind + parameter name; compiled indices and panel positions are never persistent identities. Only continuous runtime-class controls qualify; stepped/structural controls require document edits.

Mappings and normalized native bases live in version-2 SoundState. A missing version-1 extension loads Free and unassigned; a version-1 envelope carrying active D08 fields is rejected. Validation checks finite ranges, unique valid targets, graph/label/effect bounds and serialized size before any publication. The original D07 patch-size bound remains unchanged; an additional bounded8192bytes accommodates new envelope metadata.

Host playback is an overlay over saved document bases, not PatchEditor history. Existing runtime ramps and active/incoming/pending graph paths apply values; ordinary knob movement does not compile. Releasing/remapping a slot restores the latest accepted document base. The bank queue is four entries; messages include release values resolved against the latest document and the last successfully published bank. Retry coalesces latest state. Bank restores drain before normal document messages so later runtime edits/graphs win; mapped overlays apply afterward. Browser loads publish a complete new session and all new queues, preventing old lane/base messages crossing reused IDs. Delete retirement survives Undo. Explicit remapping is required for reuse.

Native GUI setters are asynchronous in nice-plug. Exact requested values are published independently of its delayed native atomics, so no host echo is required. Slot sliders, mapped knobs and closed-editor mapped CC emit native gestures. Accepted Begin/Value/End output is retried without duplicate Begin; an accepted Begin is closed even if state replacement discards its pending value. Editor close ends active GUI gestures once.

CLAP params.flush is bounded separately from process. Base/modulation pending arrays preserve flush updates until processing resumes; newer sample-zero process events supersede older matching flush updates. State publication initializes pending arrays to recalled bases and zero modulation, so an old retry mask cannot resurrect previous sound settings. Native transient modulation is cleared before setting recalled bases.

## Clock and reset

Free retains legacy behavior and standalone default. Host requires valid CLAP beats, seconds and tempo in20–300BPM. Missing/invalid transport stops Host clocks instead of inventing free-running timing. Host beats advance using tempo; Clock produces16th-note pulses with quarter-note phase. Existing wires drive sequencers/LFO sync/delay locks. Initial launch bar is four quarters (4/4), regardless of host signature; other bar signatures are not supported.

Play start, seek and loop wrap establish a new Host epoch and reset transient built-in DSP seeds/state through existing PatchEngine.reset. Sequencers restart from their reset phase, then follow host-clock pulse phase; a mid-song seek does not reconstruct arbitrary prior sequence history. Queued launches are canceled on stop; Host notes are released with source cleanup while effect/release tails continue. Restart/seek/loop reset tails. A mode change resets DSP to enter its new policy. Free MIDI notes never reset oscillators individually.

The production Timeline executes on a64-frame grid. Events land on the next boundary, giving0–63sample quantization; reported fixed plugin latency is64frames (1.333ms at48kHz), separate from device latency. Transport discontinuities at raw buffer offset0 reanchor Timeline before scheduling MIDI/parameters. Changes inside a buffer follow the same boundary rule. Native Output gain retains framework parameter handling.

nice-plug implicitly calls Plugin.reset at each start_processing. The bridge distinguishes that thread start from explicit CLAP reset. During an already-playing Host epoch, a thread restart does not create a new musical epoch. Explicit reset/activation always clear DSP, transient keys/pedals, queued MIDI/events and launch ownership; native parameter bases/modulation survive explicit reset. State recall discards transient modulation, CC pickup/button history, learned controls and old control queues.

## Bounded processing and ownership

Schedule4096 entries, raw input2048 events, native parameter list128 entries (36 reserved final value/modulation identities), MIDI ring1024 entries. Stable insertion allocates nothing; worst-case sorting remains bounded rather than constant-time. Overflow counts reach the control worker; MIDI flood overflow releases Host-owned notes instead of retaining unseen note-offs. The worker handles CC/control compilation/collection at5ms cadence even with editors closed. Graph/session destruction uses basedrop deferred collection.

No new callback allocation, deallocation, mutex, graph compile or device backend is introduced. Exported and production explicit-schedule tests guard allocations/deallocations across process/reset/ownership changes. The existing pinned framework process/start/reset mutex remains a strict no-lock acceptance failure; these tests do not prove lock freedom.

Mapped patches return KeepAlive conservatively because automation can open a silent route or extend effects beyond saved bases. Legacy unassigned projects retain finite tail estimates. REAPER owns audio devices, MIDI and processing lifecycle; no standalone audio/MIDI backend runs inside the plugin.

## Actual REAPER repeat policy

Initialize audio and defer at least one second for project/tempo setup. Disable track buffering/anticipation with I_PERFFLAGS=3 on both tracks. Close the audio device. In each fresh process, complete three36-second stereo48kHz float32 renders. Retain the first as warm-up and compare the second and third decoded PCM. Repeat in a second fresh process. No per-note seed reset or Free sound change makes these renders match.

Ordinary repetitions and earlier failed driver policies remain retained. Early startup Lua rendered before REAPER applied the tempo map; constant120 telemetry was a driver failure, not evidence that the product followed tempo. Final initialized captures exercise actual tempo changes, seek, loop, stop and delayed Free parameter readback.
