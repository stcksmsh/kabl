# Production instrument contract

REAPER controls activation, sample rate, reset, processing, MIDI and GUI lifecycle. KABL starts no CPAL/midir backend or standalone device watchdog. The pinned framework owns host parameter conversion and egui embedding. An outer CLAP entry retains the original framework plugin_data while replacing complete-state and raw-MIDI intake behavior.

The production Timeline keeps its absolute 64-frame grid and reports 64 frames of latency. Host MIDI retains sample offsets, Host source, channel and key ownership. Only MIDI dialect is advertised; CLAP-native note identity is not implemented. Expression, sustain and cleanup use D06 paths. Deactivation, reactivation and reset discard held keys and transient effects; state stores sound, not performance history.

Complete state version 1 contains PatchState and the output-gain base. Decode limits are 2 MiB, 128 modules, 512 cables, 128 label groups and eight effect modules, with additional string/parameter/step limits. Invalid identifiers, nonfinite values, unsupported versions and invalid graphs are rejected before publication. Preparation compiles a complete replacement outside audio processing. Queue rejection preserves the accepted graph and gain. Editor documents can remain visibly failed without becoming accepted persistent sound. Host gain modulation affects audio; persistence retains its unmodulated base.

Raw intake examines at most 2,048 host events, queues at most 1,024 MIDI events and forwards at most 128 parameter events. Saturated gain value and modulation are separately coalesced to their final values within the intake bound. Overflow schedules Host source loss so missed releases cannot strand held keys. The exported-entry test guards allocation/deallocation during process/reset, floods 4,097 events and covers 96 kHz reactivation.

A control worker performs compilation, editor-independent feedback and retired-graph collection. Callback work uses fixed queues and deferred destruction. Session replacement discards prior feedback. Shutdown drops queued sessions and Handles before the final collector-storage cleanup. File access and GUI frames are not required to keep controls, audio or collection alive.

Tail reporting proves finite tails only through silence-preserving audio routes gated by MIDI envelopes, adding conservative envelope/effect settling bounds. Parameter/control paths cannot prove silence. KEY-TRIGGER mode retains the maximum release bound; FullAdsr uses ten release time constants. Free-running output or values beyond the finite CLAP representation use KeepAlive with an explicit reason. Accepted rates are finite values from 1,000 through 768,000 Hz, including fractional validator rates.

The host-render policy has not achieved reproducible PCM. Oscillator reset is deterministic, but phase advances during silent processing. Fresh process and offline/online host policies differ in actual renders. No Host/Free transport synchronization or automation bank has been added to disguise this D07 gap.

The embedded toolbar uses separate control and document/action rows so Save, Save As, Perform and Routing remain distinct at the supported plugin size. This is a shared editor layout adjustment; standalone browser/input regression tests cover its interaction behavior.
