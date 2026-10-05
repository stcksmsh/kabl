# D08 R6 ownership investigation — 2026-10-05

R6 remains an unresolved acceptance failure. This investigation changes no product code or dependency. It does not constitute a successful real-time repair. Removing the reported plugin mutex alone would leave additional callback locks and unbounded retry/drain paths. A safe correction requires redesigning the CLAP framework boundary, with the concrete alternatives below.

## Exact source and lifecycle coverage

Product: `133fdb89ac56a396db0c7c961c74067782b34efb`; investigation starts from submitted `a77804dd1c73f2fcb8e291fb9ebce5553bbeb988`. nice-plug is pinned to `263b16877d0b0ad30921868338a6df46eadc9a46`; Cargo.lock resolves crossbeam-utils 0.8.23, crossbeam-queue 0.3.14 and crossbeam-channel 0.5.17. Source hashes and the probe output are retained in `evidence/r6-audit/` and the separate durable R6 archive.

The audit follows the host-facing state_bridge factory/vtable, process and flush filtering, delegated nice-plug CLAP process/start/reset/stop, wrapper construction/editor creation, activation/deactivation/destruction, native parameter update/output, track-info and state-loader callers. It inspects the called Crossbeam implementations rather than treating their names as a lock-freedom guarantee. It is not an exhaustive DSP/library/host machine-code audit, allocation stress proof or arbitrary-host concurrency test.

CLAP defines activation on the inactive main thread, start/stop/process/reset on the active audio thread, and track-info changes on the main thread. Process and parameter flush are mutually exclusive, but this does not serialize track-info notifications with audio processing. See the official [plugin lifecycle](https://github.com/free-audio/clap/blob/main/include/clap/plugin.h), [track-info contract](https://github.com/free-audio/clap/blob/main/include/clap/ext/track-info.h) and [parameter contract](https://github.com/free-audio/clap/blob/main/include/clap/ext/params.h). Current upstream headers were checked for interpretation; the product still uses its unchanged pinned clap-sys 0.5.0 dependency.

Every mutable plugin-object acquisition found in nice-plug's CLAP wrapper is:

| Caller in wrapper.rs | Ownership and consequence |
| --- | --- |
| constructor/editor, 768 | Editor is created before exposing the wrapper. Opening/closing the existing editor later uses its separate editor object, not P. |
| update_track_info_from_host, 1924 | Called by init and ext_track_info_changed (3670). Main-thread notification can overlap audio; Instrument uses the default no-op hook, but wrapper still locks P. |
| set_state_inner, 1966 | Legacy loader reacquires P for activate/reset. Called by native state load, GUI state API or process rendezvous. Outer D08 STATE extension replaces native load; RackApp does not call the framework GUI state API. Unused API presence does not eliminate the unconditional rendezvous poll. |
| activate, 2073 | Inactive main-thread construction/configuration. Allocations are allowed here. |
| deactivate, 2098 | Main-thread lifecycle after processing stops. |
| start_processing, 2115 | Audio-thread implicit reset under plugin mutex. D08 distinguishes this reset from explicit musical reset. |
| reset, 2131 | Explicit active audio-thread reset under plugin mutex. |
| process, 2430 | Mutable plugin processing under mutex. |

The task-executor mutex is separate: main/background task execution and activate-context synchronous tasks can acquire it; Instrument does not schedule background tasks from process. Editor mutexes are main/editor paths. These observations do not authorize unguarded P aliasing or eliminating lifecycle synchronization.

The reproducible compiled probe is `python3 docs/host-production/scripts/run-rt-framework-probe.py scratch/r6-audit`. It uses Cargo --locked compiler-artifact records to select the actual dependency rlibs, retains the compiler command/build log, and exits2 for the observed failed contract. It changes neither Cargo manifests nor the cache source.

## Additional whole-callback failures

1. **Fallback spin locks.** `current_audio_io_layout.load()` (2155), `current_buffer_config.load()` (2341 and parameter output at1060), and `last_process_status.store()` (2110/2440) use AtomicCell types too large for native atomic operations on this x86_64 build. Crossbeam uses global hashed SeqLocks: store acquires the write lock; load may fall back from optimistic read to a write lock. SeqLock::write retries with Backoff. The compiled probe reports AudioIOLayout120bytes, Option<BufferConfig>20bytes and ProcessStatus24bytes, all `lock_free=false`; ProcessMode1byte is lock-free. Locks are present even when usually uncontended; hashed locks can also contend across distinct objects/instances.
2. **Unconditional rendezvous mutex.** At2485, every delegated process polls the zero-capacity channel. crossbeam-channel's zero.rs try_recv279 locks its inner mutex even when empty. If GUI state is sent, process additionally deserializes/reactivates under permit_alloc and sends through a blocking rendezvous. D08 does not use that state route, but the empty poll still violates acceptance.
3. **Retrying handoffs and drain.** Native GUI events use ArrayQueue. Its pop336 contains compare_exchange_weak/backoff retry and publication-wait loops; no fixed operation bound is established. A producer paused after reservation can make the consumer wait for publication. handle_out_events1061 also drains until empty while the GUI may replenish the queue, so capacity2048 does not bound total callback work. Native parameter changes schedule GUI tasks through another ArrayQueue; the fallback main-thread check may initialize thread::current under permit_alloc if the host supplies no thread-check extension.

D08's own raw input filtering (2048 events,128 native parameter entries), fixed schedule4096 and MIDI1024 rings, bank/document limits and deferred graph retirement do not repair these delegated paths. Stop_processing only updates an atomic flag; start/reset still fail through P and status paths. Active flush avoids P but reaches buffer-config fallback locks, GUI scheduling and parameter output handoffs. Existing allocation guards establish only their exercised cases; they do not prove locks absent, do not cover the framework's permitted allocation paths and cannot establish a bounded drain under concurrent replenishment.

## Concrete correction alternatives

**Preferred: a narrowly scoped, opt-in CLAP profile in a retained ISC-licensed framework fork/vendor snapshot.** Keep the existing parameter/editor API, IDs and outer transactional state bridge. Transfer exclusive P ownership at the inactive/active lifecycle boundary using a checked nonblocking borrowing primitive, not unchecked shared mutable access. Disable the unused track-info hook for this profile or move owned notification data through a bounded handoff whose retirement remains off audio. Reject the unused generic GUI/native state-loader route and remove its audio rendezvous poll; keep kabl's existing complete-state publication as the only state route. Replace callback layout/config reads with immutable activation snapshots; publish only scalar atomic tail status. Replace GUI output with an off-audio-serialized producer and wait-free SPSC consumer, cap consumption per callback and preserve Begin/Value/End ordering and retry semantics. Coalesce audio-to-editor notifications using atomic dirty flags serviced on main, avoiding callback task queues/thread-ID initialization.

This is more than a mutex substitution: it changes object ownership, state API reachability, configuration publication, tail reporting and two event directions. Regressions must cover concurrent track notifications, state load/queue saturation, flush/process ordering, start/stop/reset and editor destruction/recreation; host automation, full restart, fresh-process render policy and eight timings must then be refreshed at the new product head. Pin original revision plus a committed patch/snapshot and upstream ISC license. Retain a source manifest and upgrade checklist. Maintenance cost: carry a CLAP profile patch and repeat the ownership/callback audit whenever upstream wrapper, event or state code changes. No cache mutation is a deliverable.

**Alternative: a kabl-specific CLAP wrapper.** Own Instrument directly at the lifecycle boundary and reuse current transactional state/raw-event bridge and scalar parameters, with bounded GUI gestures and editor notifications. This avoids generic framework state/task paths but takes ownership of audio ports, params/text conversion, GUI sizing/parenting/lifetime, latency/tail and platform integration. It has a substantially larger regression surface and ongoing CLAP/platform maintenance cost. It must preserve all18 native IDs and legacy state semantics; replacing nice-plug-egui without a compatible GUI context would expand the work further.

Neither alternative has been implemented or assigned an unsupported time estimate. A mutex-to-spinlock/try-lock substitution, AtomicRefCell substitution while retaining competing callers, or bypass through private P pointers would not satisfy the contract. No such partial fix was shipped.

## Disposition and evidence reuse

Engineering remains **submitted as draft; acceptance incomplete**. R6 is not waived, repaired or owner-approved. No dependency fork or production change was made. Baseline product, binary and complete project remain identical. Previous host/timing evidence is reused with that provenance, including ordinary-render failures and dense-editor backend errors; no new timing result is claimed. Final checks and additional fresh-process production render/recall verification are recorded separately in the R6 evidence index. Physical controller, listening, feel, latency and production-beta approval remain Kosta's checks. No merge or D09.
