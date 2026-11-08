# kabl module ABI

**Status: preview.** Until kabl 1.0 ships, nothing here is fixed. The ABI, the layout, the
metadata format and the package format may change, breaking old modules included. A breaking
change rebuilds the example modules and test files and adds a changelog line; an old module may
be refused with a "rebuild" message. From the 1.0 ship each revision only adds (optional
exports, optional meta keys) and never changes or removes anything.

This is the document the **kabl module exception** in `COPYRIGHT` names. A module that interacts
with kabl only through what is described here (the exports, the linear memory layout and the
metadata) is not a work based on kabl.

A kabl code module is a WebAssembly module (core spec, no proposals needed) that the host runs
inside wasmi. It talks to the host through exports and its own linear memory only. The reference
implementation of the host side is `crates/modules/src/code/`; the Rust SDK in `sdk/` writes the
module side for you.

## 1. Limits

| What | Limit |
|---|---|
| Blob size | 2 MiB |
| Linear memory | 4 MiB per instance, fixed after `init` (`memory.grow` while running traps) |
| Imports | none. Any import (WASI included) refuses the module |
| Settings transfer buffer (`xfer_cap`) | 64 KiB |
| Block size | the module must support blocks of 2048 frames (`max_block` word >= 2048) |
| Time | fuel per block: a module may use up to 25 % of the block period, all modules together 50 %; a wall-clock backstop applies too. Eight overruns in a row is a fault |
| Allocation | the audio thread never allocates; neither may a module need the host to |
| Clock, random, threads | none available |

Floating point is IEEE 754. The host runs wasm on the audio thread with flush-to-zero on, so
denormals cost nothing. No determinism across CPUs is promised beyond what wasm gives.

## 2. Required exports

| Export | Signature | Meaning |
|---|---|---|
| `memory` | memory | the module's linear memory |
| `init` | `(sample_rate: f32, max_block: i32) -> i32` | called once per instance; builds the module; returns the address of the layout record (section 3) |
| `process` | `(n_frames: i32)` | render `n_frames` (at most `max_block`): read inputs and params from memory, write outputs |
| `reset` | `()` | back to the freshly created state (voice reuse) |

One `process` call per block, never per sample. A trap, running out of fuel, or growing memory
disables the module (it fades out over 5 ms) and the rest of the synth keeps playing.

## 3. Layout record

`init` returns the address of nine little-endian `u32` words:

| # | Word | Meaning |
|---|---|---|
| 0 | `input` | byte offset of the first input slot |
| 1 | `output` | byte offset of the first output slot |
| 2 | `params` | byte offset of the parameter array |
| 3 | `state` | byte offset of the module's own state region |
| 4 | `state_len` | its length in bytes |
| 5 | `max_block` | frames per slot; at least 2048 |
| 6 | `in_slots` | input slot count; must equal the metadata |
| 7 | `out_slots` | output slot count; must equal the metadata |
| 8 | `n_params` | parameter count; must equal the metadata |

An io slot is `max_block` `f32` (little endian); slots lie back to back. A width-2 port takes two
consecutive slots. Ports are laid out in declaration order, inputs and outputs separately.
`params` holds one `f32` per parameter in declaration order. All offsets are 4-byte aligned,
inside memory, and the input, output and params regions must not overlap.

Per block the host writes the inputs and the parameter values, calls `process`, reads the
outputs. The host does modulation and smoothing; a module sees one value per parameter per block.

**Inputs are always finite.** The host replaces NaN and infinity in input samples with 0.0 before
the module sees them (and counts them). **A non-finite output is a fault**: the module is
disabled and its block is silence. The module must keep its output finite.

## 4. `layout_ext`: growing the layout

Optional export `layout_ext() -> i32`: the address of a length-prefixed table of `u32` words.
Word 0 is the count of words that follow. A host ignores words it does not know and a module
without the export is valid. The layout can therefore grow without another export.

Defined words (revision 1):

| Word | Meaning |
|---|---|
| 1 | `xfer_addr`: byte offset of the settings transfer buffer |
| 2 | `xfer_cap`: its size in bytes, at most 65,536 |
| 3 | `state_version`: version of the layout `state_save` writes |

A table with fewer than 3 words, or a module that lacks `state_save` or `state_load`, has no
saved settings. A buffer over the cap, or outside memory, refuses the module at load with a
message. Keep the buffer in zeroed memory (`.bss`): then it costs no blob bytes.

## 5. Saved settings

The module owns its settings format. The host moves bytes through the transfer buffer:

| Export | Signature | Meaning |
|---|---|---|
| `state_save` | `() -> i32` | write the settings at `xfer_addr`; return their length, or `-1` if they do not fit |
| `state_load` | `(len: i32, version: i32) -> i32` | the host put `len` bytes at `xfer_addr`, saved under `version`; return `1` if accepted, `0` if not (the module keeps its state) |
| `migrate` | `(len: i32, old_version: i32) -> i32` | optional. Old settings are at `xfer_addr`; write the settings for the current `state_version` there, in place; return their length or `-1` |

Rules:

- **Never truncated, never chunked.** `state_save` returning `-1` (or a length above `xfer_cap`)
  is an error: the host keeps the previously saved settings and shows a warning.
- The `.kabl` file stores, per instance, the module version and `state_version` (four
  little-endian bytes) followed by the module's bytes exactly as `state_save` wrote them.
  Settings round-trip bit-exact.
- Loading: same `state_version` goes straight to `state_load`. An older one goes through
  `migrate` (if exported), then `state_load` with the current version. A newer one (a downgrade)
  starts at defaults and the song keeps the newer settings.
- **A failure never stops the instrument.** `migrate` trapping, returning `-1` or too many bytes,
  `state_load` returning `0` or trapping, or no `migrate` for an older layout: the module starts
  at its defaults and the song keeps the old bytes, so a build that can read them finds them.
  A trap makes the host throw that instance away and build a fresh one.
- When the host saves or opens a song it calls these on the first instance of each module (lane
  0): while the song plays, `state_save` and `state_load` run at the start of an audio block under
  that module's fuel allowance (running out of fuel is a failed save or load, not a crash; a trap
  disables the module as usual), and `migrate` runs on a scratch instance off the audio thread.
  Keep them short and allocation-free.

## 6. Metadata: the `kabl.meta` custom section

UTF-8 text, one item per line; empty lines and lines starting with `#` are skipped.

```
kind acme.gain            module id stored in patches ("githubhandle.module")
version 1                 integer, starts at 1; required
abi_minor 1               optional; see section 7
name Gain                 rest of line
explain Scales it.        rest of line
category utility          source|filter|modulator|sequencer|utility|effect
rate voice                voice|global
width 4                   panel width in rack units
port in audio audio_in 1  in|out, audio|cv|unipolar|gate|pitch, name, width 1 or 2
param gain 0 4 1 linear 5 x   name min max default linear|exp|stepped|cubic smoothing_ms [unit]
advanced gain             names of params shown under "advanced"
```

`abi N` (the major) may also appear; absent means the host's own. The ids `kabl.`, `demo.` and
`test.` are reserved.

## 7. Compatibility rules (the preview rule)

- **Unknown meta keys are ignored.** A newer SDK may add optional lines; an older host skips
  them. The `cargo kabl` validator warns about unknown keys, to catch typos.
- **`abi_minor N`** states the ABI revision the module needs. The host refuses a module whose N
  is greater than its own minor (revision 1 today) with: "code module needs ABI revision N; this
  kabl supports up to M. Update kabl, or ask the module's author for a build for this version".
  A new *required* key is announced this way. No line means revision 0.
- **Unknown exports are ignored.** Optional exports a host does not know do nothing.
- **Before 1.0**, any of this may change incompatibly; see the status note at the top.
- **From 1.0**, a revision only adds: optional exports, optional meta keys, new `layout_ext`
  words. Nothing existing changes. A change that would break this goes to the owner, not to a
  release.

## 8. Songs and module identity

A song embeds the exact wasm blob of every code module it uses (identified by the SHA-256 of the
bytes, with the module id and version). Opening a song always uses its own embedded copy, so two
songs in one process may run two different versions of one module id side by side. A copy the
user declines to run, or that cannot load, becomes a stand-in: it keeps the ports and settings,
passes audio through and leaves the song's data intact.
