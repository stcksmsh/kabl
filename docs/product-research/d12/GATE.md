# D12 gate: data-section size and a `macro_rules!` `kabl_module!`

Date: 2026-11-08. Spec: `briefs/D12.md` section 6 ("Spikes needed before PR 1"); governing principle 0 of `briefs/D12-DECISIONS.md` (nothing set in stone until ship). Throwaway code, not committed: a scratch workspace outside the repo (`sdk` lib of 284 lines, `tpl` template, `bf` bitfilter-equivalent, a host harness), rustc 1.93.0, wasm32-unknown-unknown, same profile and linker flags as `crates/modules/code-examples` (opt 3, lto, `panic=abort`, `strip=debuginfo`, stack 8 KiB).

## Contradictions and new facts (read first)

Result: **PASS**, with **1** contradiction. It is with `D12.md` 3.1, not with `D12-DECISIONS.md`; nothing in the decisions file is contradicted.

1. **Contradiction (D12.md 3.1):** "`Io` ... index constants generated from the declaration". `macro_rules!` cannot mint identifiers from a const builder's strings, so named constants cannot be generated. What works: the creator writes `const CUTOFF_CV: usize = 2;` and a const assertion compares it with the builder's slot table, so a port rename or reorder is still a compile error (not built in the spike; about 30 lines). Wording to change in the brief, not the plan.
2. New fact, PR 0: state transfer needs a host-visible buffer. `state_save`/`state_load` have no defined signature and `Layout` has no scratch region. The spike put `[xfer_addr, xfer_cap, state_version]` in the `layout_ext` table and uses `state_save() -> len`, `state_load(len, version) -> 1/0`. PR 0 should fix this shape, with these rules (the spike's fixed 256 B buffer is a spike constant, not a design):
   - The module declares `xfer_cap`; the SDK sizes it from the module's declared maximum state size (a `Module` const, default 256 B), as zeroed `.bss` so it costs no blob bytes.
   - The host enforces a hard cap on `xfer_cap`, proposed **64 KiB**: it bounds per-instance memory (the 4 MiB memory cap is shared with the audio buffers, and 16 voices each carry a copy), bounds what `Embedded::keep_state` stores per instance across 4096 instances, and is far above any settings blob a module should have (the bitfilter is 32 B). A module declaring more is rejected at load with a clear message.
   - `state_save` returning `-1` or a length above `xfer_cap` is an error. The host keeps the previously saved settings and shows a warning; it never truncates and there is no chunking. The spike already returns `-1` on overflow (`StateWriter.over`).
3. New fact: `link_section` statics are emitted twice, as the custom section and again in the data segment (current examples too). Cost: 305 B (template) and 529 B (bitfilter), i.e. the meta text plus the notice text. Not fixed; far inside the targets.
4. New fact: float min/max/default stay text in the const builder (no const float formatting), so a typo like `"2OOO"` is caught by the host parse or `cargo kabl validate`, not by rustc.
5. Confirmed, already known: the current host rejects `abi_minor` (`meta line 3: unknown key`). That is PR 0's job.

## (a) Data section

Cause confirmed. One `static mut MEM` with any non-zero initial byte (params defaults, `state.version = 1`) goes to `.data`, so the whole buffer area is shipped. An all-zero static goes to `.bss` and ships nothing. Both zero-based variants give identical bytes.

| Blob | Total B | Data B | Code B | Target |
|---|---|---|---|---|
| `demo_logic.wasm` (today) | 42,758 | 41,376 | 520 | |
| `demo_bitfilter.wasm` (today) | 44,352 | 41,462 | 1,979 | |
| template, non-zero init (reproduces today) | 18,418 | 17,014 | 550 | |
| bitfilter-eq, non-zero init | 45,149 | 41,854 | 2,166 | |
| template, **zeroed** | **1,701** | 305 | 550 | <= 16 KiB |
| template, `MaybeUninit` | 1,701 | 305 | 550 | <= 16 KiB |
| bitfilter-eq, **zeroed** | **3,816** | 529 | 2,166 | <= 20 KiB |
| bitfilter-eq, `MaybeUninit` | 3,816 | 529 | 2,166 | <= 20 KiB |

Choice: zeroed (`MaybeUninit::zeroed()` in the store, defaults written in `init`/`reset`). `MaybeUninit::uninit()` gains nothing and needs more care. Further trims, not taken: the `name`, `producers` and `target_features` custom sections add about 400 B and `cargo kabl build` can strip them.

## (b) `kabl_module!` with `macro_rules!`

Compiles on rustc 1.93, no proc-macro, no crates. Creator crates carry `#![forbid(unsafe_code)]` and build: the macro's `#[no_mangle]` exports and statics do not trip the lint across a crate boundary (edition 2021; 2024 needs `unsafe(no_mangle)` inside the macro, untested). All `unsafe` is in `rt`. The template is 46 lines including meta and save/load (limit: 80 excluding DSP).

Exports emitted: `memory`, `init`, `process`, `reset`, `layout_ext`, `state_save`, `state_load`. Sections: `kabl.meta` (from the const builder, with `abi_minor 1`) and `kabl.notices` (127 B). Layout words come out as `[input, output, params, state, state_len, 2048, in_slots, out_slots, n_params]`; slot counts come from the builder, so no hand-counted `3, 2, 3`.

**Host proof, which kind:** both, honestly labelled.
- Real `WasmiEngine`, **current host with the new key stripped**: the harness removes the `abi_minor` line from `kabl.meta` (12 B) and calls `host::register_blob`. Both spike blobs load and render. The unstripped blob is rejected, as expected. Unknown exports and the extra custom section are ignored today.
- Throwaway host code **adds `layout_ext`**: a direct wasmi instance calls `init`, reads the table (`count=3`, `[xfer, 256, state_version]`), runs `state_save`, mutates, `state_load`s, saves again: bytes identical, a wrong length is rejected.
- The bitfilter-equivalent is **bit-exact** with `demo.bitfilter` over 500 blocks.

```rust
// key part of kabl-sdk (trimmed)
pub trait Module: Sized + 'static {
    const META: Meta;                                   // const builder, text in the wasm
    const STATE_VERSION: u32 = 1;
    fn new(sample_rate: f32, max_block: usize) -> Self;
    fn process(&mut self, io: &mut Io);                 // Io{input,output,params,n}: plain fields
    fn reset(&mut self);
    fn save(&self, _w: &mut StateWriter) {}
    fn load(&mut self, _v: u32, _b: &[u8]) -> bool { true }
}
macro_rules! kabl_module { ($ty:ty) => { const _: () = {
    use $crate::{rt, Module};
    const NI: usize = <$ty as Module>::META.in_slots;  // slot counts come from the builder
    const NO: usize = <$ty as Module>::META.out_slots;
    const NP: usize = <$ty as Module>::META.n_params;
    type M = rt::Mem<$ty, NI, NO, NP>;                  // zeroed: bss, not data
    static STORE: rt::Store<M> = M::store();
    #[link_section = "kabl.meta"] #[used]
    static META: [u8; <$ty as Module>::META.len] = <$ty as Module>::META.bytes();
    #[link_section = "kabl.notices"] #[used]
    static NOTICES: [u8; $crate::NOTICE.len()] = $crate::str_bytes($crate::NOTICE);
    #[no_mangle] pub extern "C" fn init(sr: f32, mb: i32) -> i32 { rt::init::<$ty, NI, NO, NP>(&STORE, sr, mb) }
    #[no_mangle] pub extern "C" fn process(n: i32)  { rt::process::<$ty, NI, NO, NP>(&STORE, n) }
    #[no_mangle] pub extern "C" fn reset()          { rt::reset::<$ty, NI, NO, NP>(&STORE) }
    #[no_mangle] pub extern "C" fn layout_ext() -> i32 { rt::layout_ext::<$ty, NI, NO, NP>(&STORE) }
    #[no_mangle] pub extern "C" fn state_save() -> i32 { rt::save::<$ty, NI, NO, NP>(&STORE) }
    #[no_mangle] pub extern "C" fn state_load(len: i32, v: i32) -> i32 { rt::load::<$ty, NI, NO, NP>(&STORE, len, v) }
}; }; }
// creator side: Meta::new("spike.gain", 1).name("Gain").port_in("audio","in",1)
//                   .port_out("audio","out",1).param("gain","0","4","1","linear",5,"x")
```

## (c) 16-voice offline render

Real `WasmiEngine` through `registry::create`, 16 instances, 64-sample blocks, 7,500 blocks (10 s at 48 kHz), under `FtzGuard::new()` on the render thread, release build, input precomputed. Each pass is 160 voice-seconds.

| Module | Wall time (2 passes) | us per voice-block |
|---|---|---|
| bitfilter-eq (SDK, zeroed) | 582 / 569 ms | about 4.8 |
| bitfilter-eq (SDK, `MaybeUninit`) | 563 / 582 ms | about 4.8 |
| `demo.bitfilter` (today's blob) | 567 ms | about 4.7 |
| template gain (SDK) | 68 / 67 ms | about 0.56 |

The SDK adds no measurable run cost. The figure is the whole engine path (host copy in and out, metering, health), and the bitfilter does two biquads on two channels, so it is heavier than the D11 kernel. 10 s of 16 voices renders in about 0.6 s. `cargo kabl test` can state "16 voices: about 0.6 s" and stay far under any CI timeout.

## Pass or fail against D12.md section 6

| Threshold | Measured | Verdict |
|---|---|---|
| Template blob <= 16 KiB | 1.7 KiB | pass |
| Bitfilter-equivalent <= 20 KiB | 3.7 KiB | pass |
| Library hides all `unsafe` | creator crates build under `forbid(unsafe_code)` | pass |
| Template `lib.rs` <= 80 lines excluding DSP | 46 lines | pass |
| Spike (b): no proc-macro, no crates, rustc 1.93 | builds; loads in `WasmiEngine` | pass |

Not measured here (later PRs): build reproducibility, fresh-agent run, hostile modules, migrate.

## Resulting PR 1 size

**Unchanged: M, about 650 lines, no split, no proc-macro crate.** The spike's 284-line `lib.rs` already covers the builder, `Io`, store, exports and runtime; it grows with the opt-in bump arena (about +80), the port-index const check (about +30), rustdoc, the two ported examples and a few host-free unit tests, plus `sdk/LICENSE`, the template LICENSE and the `deny` config. Risk drops: the blob-size and `unsafe` goals are met by construction. PR 0 gains one item (finding 2 above: define the `state_save`/`state_load` signatures and the transfer buffer in `layout_ext`), which fits its 450-line budget.
