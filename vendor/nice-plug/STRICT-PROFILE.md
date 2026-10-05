# Kabl strict CLAP profile

This is a source snapshot of Circuit-Stitch/nice-plug at
`263b16877d0b0ad30921868338a6df46eadc9a46`, retaining its ISC license. The selected
five crates share the same local core types through the root Cargo patch. The
original manifests continue to name the exact upstream revision; the committed
snapshot is the deliverable, not a modified Cargo cache. UPSTREAM-SHA256.json
records original selected source hashes. SOURCE-SHA256.json records this snapshot;
STRICT-PROFILE.patch records its changes against that revision.

The opt-in `strict-clap` feature is intentionally limited to Kabl's CLAP instrument:
application-owned state epochs, zero-sized background tasks and SysEx type, no
framework MIDI output. Unsupported generic state APIs reject explicitly; callback
tasks/output set an error flag rather than entering generic queues. Other plugin
formats are outside this profile's acceptance claim.

Checked exclusive plugin borrows and lifecycle gates use a single CAS, without
callback retries. Activation configuration is immutable while active; tail status
uses a scalar atomic. Main track notifications update framework metadata but do not
call the plugin object. Native parameter output uses an rtrb SPSC ring with a fixed
2048-record callback budget. Host rejection leaves the head pending. Producers
serialize off audio and retain overflow in an off-audio VecDeque. That backlog can
grow if a host permanently rejects delivery; callback memory and work stay bounded,
but total producer-side memory is not bounded. No accepted gesture is silently lost.
Main-thread pumping uses a fixed budget. Audio-to-editor notifications coalesce into
scalar flags and exact modulation-offset atomics.

Kabl's outer wrapper serializes raw-event producers and owns native parameter cache
writes in process/flush/start/reset. State load publishes a complete separate recall
payload; canonical pending getters, state snapshots and editor observations do not
wait for a host callback. Cache epochs protect recalled DSP defaults during a load
that races an already entered callback. CC requests also carry recall epochs.

## Upgrade audit

1. Preserve upstream revision, license and all selected-file hashes before updating.
2. Rebase this narrow patch; inspect every CLAP plugin-object acquisition and each
   competing editor, state, track-info and lifecycle caller.
3. Recheck AtomicRefCell single-attempt mutable borrow and rtrb publication/pop code;
   do not infer a real-time bound from queue capacity or an atomic API name.
4. Inspect process, active flush, start, stop and reset transitive paths for locks,
   retries, allocation/destruction and generic task/state/thread-ID reachability.
5. Run the compiled primitive probe and adversarial plugin regressions, including
   paused producers, replenishment, rejected gesture heads and concurrent recall.
6. Repeat strict Clippy, workspace tests, release/validator and actual REAPER state,
   automation, closed-editor controls, transport, lifecycle, render policy and all
   eight timing cases. Obtain independent review of the actual new product head.

Maintenance cost is retaining this opt-in CLAP patch and repeating that audit when
upstream wrapper, state, editor, events, atomics or ring implementations change.
