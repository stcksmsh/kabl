# Strict profile callback audit

Product: f7157619836075d0d7704e59be5d66e6224890b1. Production Linux x86_64
binary: 3baa8ee3cc0f2f99e1823e0a741d7301713098f99e92dc84f7556b8d0483aca4.

Coverage follows the exported Kabl outer CLAP callbacks into the opt-in vendored
nice-plug profile, then the called ownership/ring/atomic implementations and Kabl
state/editor/worker callers. It covers valid CLAP lifecycle/input contracts for
this concrete Instrument, with `BackgroundTask=()`, `SysExMessage=()` and no generic
MIDI output. This is a source/reachability audit plus exercised allocation guards,
not an exhaustive machine-code/host/OS timing proof or arbitrary hostile-host proof.

| Entry/path | Ownership and bound | Lock/allocation/destruction disposition |
| --- | --- | --- |
| process_bridge | Checked outer exclusive guard before raw UnsafeCell producers; raw input2048, native128, fixed rings, CC16 | No lock/wait/retry; overflow counted; no dynamic input copies |
| delegated process | Processing-phase gate; checked exclusive P; at most filtered input128 automation splits | Plugin mutex replaced; oversized AtomicCell reads replaced by immutable activation snapshots; no generic state channel poll |
| active/inactive flush | Same outer guard + delegated phase gate; native128 input and output2048 records | Process/flush serialized by CLAP and enforced against overlap; no P alias; no callback queue lock |
| start/reset | Outer native-cache ownership; delegated active/processing gate + checked P; existing bounded DSP/ring reset | No plugin/status lock; native recall18 scalar writes; no state deserialize/reactivation on audio |
| stop | Delegated processing-phase gate, scalar processing state | No plugin lock or allocation |
| parameter output | Off-audio serialized producers; rtrb checked consumer; fixed2048-record budget including stale records | Consumer never accesses producer mutex; host rejection retains head; Copy payloads have no destructor |
| notifications | Scalar dirty flags; fixed per-parameter modulation atomics; asynchronous host request_callback | No callback task queue or thread-ID initialization; editor callbacks and pumping occur on main |
| state load/getters/editor | Main Control mutex prepares session/recall; callback alone writes native cache; canonical pending overlay covers18 controls | Main never competes for P/native mutable cache; epoch guards retain old or recalled scene during interleaving |
| controller output | Worker publishes dedicated exact value + epoch under main/control ownership; callback16 slots | Stale values retire; accepted Begin receives End; valid unsynced values retain pending notification |
| graph retirement | Existing basedrop Owned retirement queues node with atomic tail swap/next store | Session/graph/ring destruction deferred to control collector; collector traversal is off audio |

AtomicRefCell0.1.14 mutable try-borrow uses one strong compare_exchange. No callback
spinning or borrow retry was added. rtrb0.4.0 publishes after writing data, then
consumer peek/pop uses bounded cursor operations; an unpublished producer record
appears empty, without reservation spinning. Activation snapshots are written only
under inactive lifecycle ownership. Scalar process mode and tail status are native
atomics on this target; the failure probe retains the old large AtomicCell negative
controls and tests actual repaired primitives rather than suppressing the failure.

Main track-info notification can overlap audio: it updates host metadata only and
never borrows P. Editor creation precedes wrapper exposure; GUI destruction/recreation
uses independent main/editor objects, not P. Host must serialize lifecycle, process
and flush as CLAP specifies, keep plugin alive while calling it, and dispatch
request_callback asynchronously on main. Gates reject invalid overlap without unsafe
mutable aliasing; they do not make arbitrary invalid host pointers or destruction
races safe.

Unsupported generic state routes fail explicitly. Callback generic task/MIDI output
routes do not enter queues in this profile; this concrete Kabl type does not call
them. Activation/main/background/editor preparation may allocate and lock. Producer
overflow uses an off-audio pending VecDeque that can grow if the host never accepts
output; total GUI-side memory is not claimed bounded. Callback storage/work remain
bounded and accepted gestures are retained.

Focused checks cover paused producer mutex, continuous replenishment, rejected
Begin/Value/End, stale GUI queue after recall, old callback native writes after a
barrier-separated recall, old CC mask retried after recall with an open gesture,
paused main track notification during64 audio blocks, start/stop/reset/reactivation,
active flush/process precedence, generic state rejection and existing state/graph
compatibility. Actual REAPER lifecycle/render/timing evidence is indexed separately.
