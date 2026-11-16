# Writing a kabl module

**Preview.** Until kabl 1.0 the ABI, the metadata format, the package format, this SDK and the
`cargo kabl` commands may change, breaking old modules. When one does, rebuild your module. This
file was checked by running every command in it.

A kabl module is a small WebAssembly program that kabl runs in a sandbox: no file, network, clock,
random or thread access, a hard memory and time budget, and a fault in it never stops the synth.
You write it in Rust with `kabl-module-sdk` (this directory, licence 0BSD, no dependencies, no
`unsafe` in your code). You build and check it with `cargo kabl` (GPL-3.0-or-later, `crates/cargo-kabl`).
The wire contract, for other languages, is `docs/sdk/ABI.md`; section 10 shows it in C.

## 1. Install

You need `rustup` (Rust from rust-lang.org) and a kabl checkout.

```
cargo install --path crates/cargo-kabl      # from the kabl checkout; installs `cargo kabl`
cargo kabl --help
```

`cargo install` puts `cargo-kabl` in `~/.cargo/bin`, which rustup already has on your `PATH`.
With `--root DIR` it goes to `DIR/bin`: put that on `PATH`, or `cargo kabl` is not found.

Nothing else: your new project pins its own compiler (`rust-toolchain.toml`, Rust 1.93.0) and
`rustup` fetches that and the `wasm32-unknown-unknown` target the first time you build in it. If
you prefer to do it by hand, `rustup target add wasm32-unknown-unknown`.

## 2. Make a module

```
cargo kabl new yourhandle.softclip --sdk-path /path/to/kabl/sdk/kabl-module-sdk
cd yourhandle-softclip
cargo test              # the template's own unit test, on your machine
cargo kabl build        # wasm blob + yourhandle.softclip-1.kablmod
```

`new` creates the project folder in the current folder, named after the id with the dot turned
into a dash (`yourhandle-softclip`). `--into DIR` makes it inside `DIR` instead
(`DIR/yourhandle-softclip`). Every command also works from outside the folder with
`--manifest-path yourhandle-softclip/Cargo.toml`; `cargo test` takes the same flag.

The id is `githubhandle.module`: lowercase letters, digits, `_` and `-`, exactly one dot. The
prefixes `kabl.`, `demo.` and `test.` are reserved. `--control` starts from a control-signal
template instead of an audio effect. Without `--sdk-path` the new project depends on the SDK
through git (`https://github.com/stcksmsh/kabl`), which needs that repository to be readable by
you. You get `Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`, `README.md` and a 0BSD `LICENSE`
(keep it or put your own; the SDK you link is 0BSD).

Before you ship, fill in `Cargo.toml`: `authors`, `description`, `license` (an SPDX id) and, for a
copyleft licence such as GPL, `repository`. They go into the package manifest.

## 3. The module file

`src/lib.rs` is about 100 lines. Here is the template (a gain, struct `Gain`, one param `gain`)
rewritten as a soft clipper, so the struct is already renamed `Softclip`, there are two params and
the id, `explain` and category differ. In your own copy rename `Gain` and `GAIN` by find and
replace, then edit `META` and `process`. Leave the template's three `fault-*` blocks in `process`
(section 5 says why); they are marked `Wasm only` there because a native `cargo test` must not
hit them.

```rust
#![cfg_attr(not(test), no_std)]   // wasm has no std; `cargo test` runs on your machine with std
#![forbid(unsafe_code)]
use kabl_module_sdk::*;

struct Softclip {
    last: f32,                     // anything the module remembers lives in a plain struct
}

// Indices are slot numbers in declaration order. The `const _` block below turns a rename or
// reorder into a compile error instead of a silent swap.
const IN: usize = 0;
const OUT: usize = 0;
const DRIVE: usize = 0;
const LEVEL: usize = 1;

impl Module for Softclip {
    const META: Meta = Meta::new("yourhandle.softclip", 1)   // id, version (integer, starts at 1)
        .name("Soft clip")
        .explain("Squashes peaks smoothly. Drive adds more; level sets the output.")
        .category(Category::Effect)
        .rate(Rate::Voice)
        .width(4)
        .port_in(PortType::Audio, "in", 1)
        .port_out(PortType::Audio, "out", 1)
        .param("drive", "1", "16", "1", Taper::Exp, 5, "x")     // name min max default taper smoothing_ms unit
        .param("level", "0", "1.5", "1", Taper::Linear, 5, "x");
    const STATE_CAP: usize = 4;    // bytes `save` may write (here one f32)

    fn new(_: &mut Init) -> Self { Softclip { last: 1.0 } }
    fn reset(&mut self) { self.last = 1.0; }                    // back to the fresh state

    fn process(&mut self, io: &mut Io) {
        let (drive, level) = (io.param(DRIVE), io.param(LEVEL));
        for i in 0..io.n {                                      // only `..io.n` frames are valid
            let x = io.input[IN][i] * drive;
            let abs = if x < 0.0 { -x } else { x };
            io.output[OUT][i] = x / (1.0 + abs) * level;        // stays inside -1..1
        }
        self.last = drive;
        // ... the template's three `#[cfg(feature = "fault-...")]` blocks stay here ...
    }

    fn save(&self, w: &mut StateWriter) { w.f32(self.last); }
    fn load(&mut self, _version: u32, bytes: &[u8]) -> bool {
        match StateReader::new(bytes).f32() {
            Some(v) => { self.last = v; true }
            None => false,
        }
    }
}

const _: () = {
    Softclip::META.assert_in(IN, "in");
    Softclip::META.assert_out(OUT, "out");
    Softclip::META.assert_param(DRIVE, "drive");
    Softclip::META.assert_param(LEVEL, "level");
};

kabl_module!(Softclip);            // emits every export, the memory layout and the metadata
```

The template's own test is the pattern for yours (section 5).

What the pieces mean:

- **`META`** is built when you compile, so a module cannot disagree with its own metadata.
  `rate`: `Rate::Voice` makes one instance per playing voice (use it for effects and anything
  per-note); `Rate::Global` makes one for the whole synth (clocks, sequencers, modulators).
  `width` is the panel width in rack units. `category`: Source, Filter, Modulator, Sequencer,
  Utility, Effect. `.advanced("param")` tucks a param behind the panel's expander.
- **Ports** are `port_in` / `port_out(type, name, width)`; width is 1 or 2 (two slots, for stereo).
  Types: `Audio` (-1..1 nominal), `Cv` (-1..1), `Unipolar` (0..1), `Gate` (0 or 1; treat above 0.5
  as high), `Pitch` (float semitones, 1 V/octave semantics). Input and output slots are numbered
  separately, in the order you declare them; a width-2 port takes two slots.
- **Params** are decimal text for min, max and default (`"0.5"`), a taper (`Linear`, `Exp`, `Stepped`
  for a switch of whole numbers, `Cubic`), a smoothing time in ms and an optional unit label.
  The host smooths and modulates them: `io.param(i)` is one value per block. Read it as a float and
  round it yourself for a stepped param (`(v + 0.5) as i32`).
- **`io`**: `io.n` frames in this block, at most 2048. `io.input[slot][i]` and `io.output[slot][i]`
  are `f32`; `io.input(slot)` and `io.output(slot)` give slices of exactly `n` frames. Write every
  output frame, every block.
- **Inputs are always finite** (kabl replaces NaN and infinity with 0). **Outputs must be
  finite**: a NaN or infinity disables the module. Divide by something that can be zero, take a
  log or a square root of something that can go negative, or feed back unboundedly, and you get a
  fault. Clamp.
- **Maths**: no `std`, no `libm`. The crate gives `flush(x)` (kills denormals), `exp2(x)`,
  `sin_cos(x)`. Anything else you write yourself (`x / (1 + |x|)` above is a soft clip with no
  library). `abs`, `min`, `max`, `clamp` and `to_bits` work; `sqrt`, `sin`, `exp`, `ln`, `powf`,
  `floor`, `trunc` and `rem_euclid` do not exist (`no method named `sqrt` found for type `f32``).
  To truncate use an `as i32` cast (clamp first: a huge float cast saturates, it does not trap),
  and for a fraction `x - (x as i32) as f32`.
- **Memory**: every buffer is a static, no allocator, no `Vec` or `Box`. Keep fixed arrays in your
  struct. Big delay lines or tables: enable the `arena` feature in `Cargo.toml`
  (`kabl-module-sdk = { ..., features = ["arena"] }`), set `const ARENA_BYTES: usize = N;`, and take
  slices from `init.arena.alloc_slice::<f32>(n)` in `new`. Your whole memory is capped at 4 MiB.
- **Voices**: with `Rate::Voice` kabl builds one instance per voice from `new`, so state in your
  struct is per voice. Do not keep state in a `static`.

kabl plays up to 16 voices. A module that must exist once (a global clock) says `Rate::Global`.

## 4. Saved settings

`save` writes whatever must survive a save and reload of a patch; `load` reads it back. Write only
what the knobs do not already hold (knob values are saved by kabl itself): counters, which step a
sequencer is on, a recorded table. `STATE_CAP` is the most you may write (at most 64 KiB); writing
past it makes the save fail and kabl keeps the previous settings. `StateWriter` has `u32`, `f32`
and `bytes`; `StateReader` has the matching reads, each returns `None` if the data is short. A
module with nothing to save leaves `save`/`load` out.

When you change what `save` writes, set `const STATE_VERSION: u32 = 2;` (the default is 1) and
implement `fn migrate(old_version: u32, bytes: &[u8], out: &mut StateWriter) -> bool` to turn old
bytes into the new layout. Without `migrate`, an old patch starts your module at its defaults and
keeps the old bytes inside the file. A `load` that returns `false`, or a trap, does the same;
the instrument never stops.

Bump the module `version` in `Meta::new(id, version)` whenever the ports or params change in a way
patches could notice. A patch keeps its own copy of the exact version it was saved with.

## 5. Test and build

```
cargo test              # your `#[cfg(test)]` unit tests, on the host
cargo kabl test         # build, then the offline checks below
cargo kabl build        # the same build; writes target/kabl/<id>-<version>.kablmod
cargo kabl validate target/wasm32-unknown-unknown/release/kabl/<id>.wasm
```

Unit tests are ordinary Rust tests (with `std`) in a `#[cfg(test)] mod tests { use super::*; ... }`
at the bottom of `lib.rs`:

```rust
let mut m = Softclip::new(&mut Init { sample_rate: 48_000.0, max_block: 64 });
let mut input = [[0.0; MAX_BLOCK]];            // one array per input slot
let mut output = [[0.0; MAX_BLOCK]];           // one per output slot
input[IN][..4].copy_from_slice(&[1.0, -0.5, 0.25, 0.0]);
let mut io = Io { n: 4, input: &input, output: &mut output, params: &[4.0, 1.0] };
m.process(&mut io);
assert!(output[OUT][..4].iter().all(|v| v.abs() < 1.0));
```

`cargo kabl test` runs your wasm in the same engine kabl uses, under the same float settings:

| Check | What it proves |
|---|---|
| golden hash | the output on a fixed noise input is unchanged; first run records `kabl-golden.txt` (commit it), `--bless` accepts an intended change |
| determinism | two instances give identical bytes |
| parameter sweep | every param at five points: output finite, no fault |
| reset | the same input after other input gives the same output |
| state round trip | `save`, `load`, `save` give identical bytes |
| 16 voices | 16 instances render exactly like one |
| cost | percent of the block period at 48 kHz, one voice and 16 (an estimate: fuel to time varies about 2x; the budget is in the limits table) |
| fault-trap, fault-spin, fault-nan | kabl stops a trapping, spinning and NaN-producing build, naming the fault |

The three `fault-*` checks build your crate with each cargo feature from `Cargo.toml` and need the
template's three `#[cfg(feature = "fault-...")]` blocks in `process`. Keep both, or delete both.

`cargo kabl build` also validates: no imports, required exports, metadata, memory and size,
and a smoke render (silence, impulse, full-scale square, large and denormal inputs, at your
minimum, default and maximum params). It writes the Rust toolchain's licence notice into the blob
for you, and builds with fixed flags and remapped paths, so two builds on the same toolchain are
byte-identical. Use `--locked` for CI. `--manifest-path FILE` and `-p CRATE` pick the crate.

A `.kablmod` is a zip: `module.wasm`, `manifest.json` (from your `Cargo.toml` and the blob),
and any of `README.md`, `LICENSE`, `CHANGELOG.md` that sit next to `Cargo.toml`. It cannot carry
code that is not the wasm. There is no custom panel artwork yet: kabl draws the module's face from
its ports and params (custom panels come with 1.0).

## 6. Load it in kabl, save, reload, share

1. Start kabl (`cargo run --release -p kabl-ui` in the checkout).
2. **Drop the `.kablmod` file onto the kabl window.** The status line says
   `Installed yourhandle.softclip v1 (0BSD, by <authors>).` The first install also shows what publishing
   a package permits: the unmodified module code may travel inside songs that use it. Installed
   packages live in `modules/` inside kabl's user directory (`$KABL_USER_DIR`, else
   `$XDG_DATA_HOME/kabl`, else `~/.local/share/kabl`), one file per module version. Removing a
   file there uninstalls it.
3. **Add module**: search for your module's name. Add it, wire it between a source and the
   output, move the knobs.
   The module's face is drawn from its ports and params: `width(4)` rack units holds a few knobs;
   a module with many params needs a larger width (try it in the app).
4. **Save** with Ctrl+S (or Save / Save As in the menu on the sound's name in the toolbar). You
   name the sound; the file is a `.kabl` patch and contains a copy of your module's code and its
   saved settings.
5. **Reload**: open the patch again (Browse sounds). The first time a patch with custom code opens,
   kabl asks: **Open** runs the code, **Cancel** opens the patch with the module as a pass-through
   stand-in. Your knobs and saved settings come back exactly.
6. **Share**: send the `.kablmod` (the other person drops it on their window, the module then
   shows in Add module), or send only the `.kabl` patch: the code travels inside it and works
   without any install. If a patch uses a version of your module that is not installed, kabl tells
   you `<id> v<N> is not installed; this patch runs its own copy of the code`.

A new version: raise `version`, rebuild, ship the new `.kablmod` (installing a second version
beside the first is fine; the same id and version with different code, or the same id from another
author, is refused).

## 7. Limits

| What | Limit |
|---|---|
| Blob | 2 MiB; a package 4 MiB. A template module is about 6 KiB |
| Memory | 4 MiB per instance, fixed after `init`; `memory.grow` while running is a fault |
| Time | one module may use 25 % of the block period, all modules together 50 %; eight overruns in a row disable it |
| Block | `io.n` is at most 2048; do not assume a size, do not assume it is constant |
| Ports / params | at most 16 ports of each direction, 32 params, port width 1 or 2 |
| Metadata text | 2048 bytes (long `explain` text counts) |
| Saved settings | `STATE_CAP` at most 64 KiB, written whole (never chunked) |
| Imports | none at all; WASI included |
| Clock, random, files, network, threads | not available. Derive time from `Init::sample_rate` and frame counts; keep your own PRNG state |
| Floats | IEEE 754 f32; denormals are flushed by the host; identical across voices, not promised across CPUs |
| Ids | `githubhandle.module`; `kabl.`, `demo.`, `test.` reserved |

## 8. When something goes wrong

Each row: what you see, why, what to do. Texts are the real ones; parts in `<angle brackets>` vary.

### 8.1 Creating and building

| You see | Why | Fix |
|---|---|---|
| `id `x` must be `githubhandle.module` (lowercase letters, digits, `_`, `-`)` | the id has capitals, spaces or not exactly one dot | `cargo kabl new yourhandle.name` |
| `id `demo.x` uses the reserved prefix `demo.`` | `kabl.`, `demo.`, `test.` belong to kabl | use your own handle |
| `<dir> already exists` | `cargo kabl new` never overwrites | pick another name or `--into DIR` |
| `error: `#[panic_handler]` function required, but not found` from `cargo build` | plain `cargo build` compiles for your computer; a module is `no_std` wasm | use `cargo kabl build` (and `cargo test` for unit tests) |
| `no method named `sqrt` found for type `f32`` (also `sin`, `exp`, `ln`, `powf`) | those need `std` | write your own, or use `exp2` and `sin_cos` from the SDK |
| `<path>: No such file or directory` on build | wrong `--manifest-path` | point at your `Cargo.toml` |
| compile error `evaluation panicked: input port is at a different slot` (also `output port ...`, `param is at a different index`, `no such input port`, `no such param`) | an index constant disagrees with the order in `META` | fix the constant or the order |
| compile error `too many params`, `too many input ports`, `meta text too long`, `port width is 1 or 2`, `empty name`, `no line breaks in text` | a `META` limit from section 7 | shrink `META` |
| compile error `STATE_CAP is over the 64 KiB host limit` / `ARENA_BYTES needs kabl-module-sdk's arena feature` | state or arena setting | lower it / enable the feature |
| `cargo kabl: cargo built no cdylib .wasm: is `crate-type = ["cdylib"]` set?` | `[lib] crate-type` removed | restore it from the template |
| `cannot tell which package is the module; pick one with `--package`` | a workspace with several crates | `-p yourcrate` |
| `no `kabl.notices` section: build with `cargo kabl build`...` (warning) | the blob came from plain `cargo build` | ship the output of `cargo kabl build` |
| `meta: unknown key `x` (the host ignores it; a typo?)` (warning) | a metadata line kabl does not know | fix the typo |
| `output reaches <v> on an input of +-1 or less ...` (warning) | a blow-up, for example feedback gain above 1 | limit the gain or clamp |
| `kabl stops this module if an input reaches about +-1e20 ...` (warning) | large inputs make the output infinite | guard huge values if it matters |
| `smoke render: output is not finite (input: <stimulus>, params: <set>); the host disables a module that does this` | NaN or infinity out on realistic input | clamp divisions, roots, logs |
| `smoke render: module trapped` / `ran out of time budget` / `used too much memory` ... `(input: ..., params: ...)` | the module faulted on that input | section 8.3; fix the code path |
| `validation failed; no blob written` | any error above | fix it, run again |
| `<crate>: <message> (set it in Cargo.toml: authors, license, repository)` | package manifest rule: `authors`, `license` required; `repository` required for GPL and other copyleft | set those fields |

### 8.2 `cargo kabl test`

| You see | Why | Fix |
|---|---|---|
| `FAIL  golden hash: output hash is <h>, <path> has `<id> <h2>`: the module's output changed; rerun with --bless if that is intended` | the output differs from the recorded one | if you meant it, `cargo kabl test --bless` |
| `FAIL  parameter sweep: param `p` = <v>: output is not finite` | some param value gives NaN or infinity | clamp |
| `FAIL  reset: the same input after other input gives different output: reset leaves state behind` | `reset` does not restore everything | clear all state in `reset` |
| `FAIL  determinism: two instances gave different bytes` | state outside the struct, or uninitialised data | keep all state in the struct |
| `FAIL  state round trip: load refused the bytes its own save wrote` / `save after load differs from the saved bytes` / `save returned <n> for a <cap> byte buffer` | `load` rejects `save`'s output, or `save` is not exact, or `STATE_CAP` is too small | make them inverse; raise `STATE_CAP` |
| `FAIL  16 voices: voice <v> differs from a solo instance on the same input` | state shared between instances | no `static` state |
| `FAIL  fault-trap: was not stopped` (also `fault-spin`, `fault-nan`) | the `fault-*` blocks are missing from `process` or the features from `Cargo.toml` | keep both or remove both |
| `<n> check(s) failed` | summary | fix the FAIL lines above it |

### 8.3 In kabl

| You see | Why | Fix |
|---|---|---|
| status `Module not installed: <reason>` | a `.kablmod` was refused. Reasons: `the manifest has no `licence``; `licence <spdx> is copyleft: the manifest needs a `source` URL`; `the manifest's sha256 does not match module.wasm`; `built for ABI <n> but this kabl runs ABI 0`; `the manifest says <id> v<n> "<name>" but the wasm says ...`; `not a valid package: <what>`; `the package is over the size cap` | rebuild with `cargo kabl build`; set `license`/`repository` |
| `<id> is already installed from <a>; a package from <b> cannot replace it` | first install of an id wins | use your own id |
| `<id> v<n> is already installed with different code (installed <h>, new <h2>)` | same id and version, different bytes | bump `version`, or remove the old file in `modules/` |
| `invalid code module: missing `kabl.meta` section` / `missing export `init`` | not built by the SDK; a C build missed the section or exports | section 10, or use `cargo kabl build` |
| `code module imports `<name>`; only kabl's ABI is allowed` | the wasm imports something (WASI, `env`) | no imports: do not use `std`, `alloc` or a crate that needs them |
| `code module is <n> bytes; the cap is 2097152` | blob over 2 MiB | drop big `static` data |
| `code module wants <n> bytes of memory; the cap is 4194304` | memory over 4 MiB | smaller buffers |
| `code module targets ABI v<n>; this kabl runs v0` | built for another ABI | rebuild with this SDK |
| `code module needs ABI revision <n>; this kabl supports up to <m>. Update kabl, or ask the module's author for a build for this version` | a newer or older SDK than kabl | match SDK and kabl versions |
| `code module layout: <what>` (`layout input slot count differs from metadata`, `layout regions input and output overlap`, `module supports blocks of <n> frames, host needs 2048` ...) | hand-written layout (C, other languages) disagrees with the metadata or the block size | section 10; SDK users never see this |
| `invalid code module: meta line <n>: <what>` (`bad version`, `unknown category`, `rate must be voice or global`, `port needs: in|out type name width`, `param needs: name min max default taper smoothing_ms [unit]`, `unknown taper`, `bad number `x` for min`) | a metadata line is malformed | the builder prevents these; if you wrote the section by hand, fix the line |
| `invalid code module: meta: `version` is required and starts at 1` / `empty kind` / `duplicate port `p`` / `duplicate param `p`` / `param `p` has an invalid range` / `port `p` width 2 (must be 1 or 2)` | metadata sanity | fix `META` (min must be below max) |
| the module's face shows a badge `trapped`, `ran out of time budget`, `used too much memory`, `produced NaN or infinity`, `kept overrunning its time budget` or `has an unusable buffer layout`; it fades out in 5 ms and the rest of the synth keeps playing | a runtime fault (`memory.grow`, out-of-bounds access, `panic!`, divide by zero, a loop that is too long, eight slow blocks in a row, non-finite output) | reproduce with `cargo kabl test`; use `cargo kabl test` cost figure; bound loops by `io.n` |
| opening a patch: the module is a pass-through stand-in | you chose Cancel, or the embedded code cannot load (rows above) | reopen and choose Open; rebuild the module |
| a settings warning when saving, or a module at defaults after opening | `save` wrote more than `STATE_CAP` or returned -1; `load` returned `false`; the patch holds settings of a newer `STATE_VERSION` | raise `STATE_CAP`; write a `migrate`; the old bytes stay in the patch |

## 9. Reference

- `cargo doc --open -p kabl-module-sdk` in `sdk/` has every type: `Module`, `Meta`, `Io`, `Init`,
  `StateWriter`, `StateReader`, `Bump` (feature `arena`), `flush`, `exp2`, `sin_cos`.
- `sdk/examples/logic`, `sdk/examples/bitfilter`: two complete modules with gates, CV and state.
- `docs/sdk/ABI.md`: the exports, memory layout, metadata keys and the compatibility rules.

## 10. A module in C

The ABI is language-neutral; only Rust has a crate and a template. `sdk/examples/c-gain/gain.c` is
a whole module in about 55 lines: no libc, no imports, every buffer a static. It needs `clang`
with the wasm32 backend and a wasm linker; `build.sh` uses `wasm-ld` if it is on `PATH`, and
otherwise the `wasm-ld` that ships inside the Rust toolchain:

```
sh sdk/examples/c-gain/build.sh                 # writes sdk/examples/c-gain/c-gain.wasm
cargo kabl validate sdk/examples/c-gain/c-gain.wasm
cargo test -p cargo-kabl --test c_example       # loads it in kabl's engine, checks the gain
```

You must provide: the exports `init(f32, i32) -> i32`, `process(i32)`, `reset()` and `memory`;
a `kabl.meta` custom section (C cannot make one from a variable, so `gain.c` writes it with
`.section .custom_section.kabl.meta,"",@` in file-scope assembly); `init` returns the address of
nine `u32` words (input, output, params, state, state_len, max_block, in_slots, out_slots,
n_params; ABI.md section 3). The warning about `kabl.notices` does not apply to C: the section
carries the Rust toolchain's notices, and your module has none.
