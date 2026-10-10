# Review

Fresh `code-reviewer` subagent, read-only, against base 2195de0 and head 488114e. Limits stated by the reviewer: Clock internals read only partly; `PatchEngine` queue ordering and the swap path not reviewed; UI rendering not reviewed.

| Finding | Severity | Resolution |
|---|---|---|
| `Settings::set` indexes out of range for a slot >= 34 (panic on the audio thread if a future caller sends one) | high (latent) | Fixed: slots >= `SLOTS` are ignored; the voice-lane test also sends slot 255. |
| Epoch is discarded when computing the pulse in effect; correct only because a new epoch starts at tick 0 | medium | Comment added at the site naming the invariant (`Clock::process`). |
| Pattern edit on a bypassed route is not sent as a runtime value | medium | No change: bypass changes compile, a bypassed route has no node, and the next bypass-off compile reads the stored params. |
| No test for voice-rate sources | low | Added `voice_rate_cable_gates_every_lane_and_takes_runtime_edits`. |
| `slot_name` allocates (used by `is_functional` in `runtime_changes`) | low | Deferred: control thread only, once per cable per edit. |
| `RouteSlot.step` placeholder 0 | low | Not changed: pre-existing pattern in `compile.rs`. |

Legacy hashes, timing and claims in the tests were checked by the reviewer and found sound. Reviewer approval is not Kosta's acceptance.
