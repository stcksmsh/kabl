# Single-file `.kabl` patch format: research

Research only. No format change, no new dependency in any shipped crate. Reproduce with `docs/format/bench_codecs.py <repo>` (Python, `pip install zstandard brotli cbor2 msgpack`) and `cargo run --release -p kabl-core --example load_bench -- <repo>` (benchmark-only example, uses existing deps). Timings are from one busy desktop machine, so treat them as order-of-magnitude (the held-out rows were noisy).

## 1. Corpus

141 patch folders (129 factory/test patches plus demos/docs fixtures; `load_bench` loads 139 of them).

| Part | Bytes | Share of log+checkpoint |
|---|---|---|
| `log.jsonl` | 4,482,841 | 69 % |
| `checkpoint.json` (pretty-printed) | 2,057,003 | 31 % |
| `meta.toml` | 2,679 | 0 % |
| of the log: `SetTable` (embedded wavetables) | 131,359 | 2.9 % of log, in **one** patch (`imported-morph`) |

Median log is 25 KB / 115 ops; the largest is 194 KB / 923 ops. Of the log, `inverse` fields are 32 % of the raw bytes (4.48 MB → 3.04 MB without them). So today: the log dominates, the checkpoint is a redundant third, and wavetables are a non-issue for the current corpus (they would dominate a patch that imports many tables; not measured).

Current I/O (`crates/core/src/format.rs`): `save` rewrites all three files with plain `fs::write` (not atomic, a crash mid-save can leave a log and checkpoint that disagree); `load` fully replays the log and ignores the checkpoint. CLAP state (`state_bridge.rs`) is separate: it serialises `SoundState` as JSON behind a u64 length header, capped by `MAX_STATE_BYTES`; it never touches the log.

## 2. Codecs (per patch, whole corpus, log only)

Ratio = raw / compressed, summed over 141 patches. Times are mean ms per patch.

| Payload | Codec | Compressed | Ratio | Enc ms | Dec ms |
|---|---|---|---|---|---|
| JSON (as today), 4.48 MB | deflate 9 (flate2 baseline) | 468,639 | 9.6 | 0.57 | 0.05 |
| | zstd 3 | 478,888 | 9.4 | 0.05 | 0.08 |
| | zstd 19 | 400,212 | 11.2 | 13.6 | 0.03 |
| | zstd 22 `--long` | 399,808 | 11.2 | 16.1 | 0.03 |
| | xz 9e | 380,824 | 11.8 | 31.5 | 0.21 |
| | brotli 11 | 365,374 | 12.3 | 38.6 | 0.06 |
| JSON without `inverse`, 3.04 MB | zstd 19 | 338,174 | 9.0 | 8.0 | 0.02 |
| | xz 9e | 323,548 | 9.4 | 27.5 | 0.17 |
| | brotli 11 | 307,334 | 9.9 | 24.7 | 0.05 |
| CBOR, 3.29 MB | zstd 19 | 417,148 | 7.9 | 7.2 | 0.02 |
| msgpack, 3.26 MB | zstd 19 | 415,784 | 7.8 | 7.5 | 0.02 |
| CBOR without `inverse`, 2.24 MB | zstd 19 | 354,204 | 6.3 | 4.2 | 0.02 |
| | brotli 11 | 328,906 | 6.8 | 17.3 | 0.05 |

Held-out dictionary test (dictionary trained on even-numbered patches, scored on odd ones, 1.98 MB raw JSON):

| Codec | Compressed | Ratio |
|---|---|---|
| zstd 19 | 230,815 | 8.6 |
| zstd 19 + 112 KB dictionary (actual trained size 48 KB) | 202,412 | 9.8 (-12 %) |
| brotli 11 | 212,080 | 9.3 |

Findings:
- zstd 19 gives about 11x; level 22 and `--long` add nothing (patches are tiny). zstd 3 is ~20 % worse. All decode in well under 0.1 ms per patch, so decode speed does not matter at this size.
- brotli/xz beat zstd 19 by 5-9 % without a dictionary, at 2-4x the encode time. A trained zstd dictionary beats brotli on held-out data (-12 % vs -8 %) and also fixes the small-frame problem below. A dictionary is the best single lever.
- **A binary op encoding loses.** CBOR/msgpack is 7 % smaller raw but 4-23 % *larger* after compression (354 KB vs 338 KB at the same content; 417 KB vs 400 KB with `inverse`), because zstd already models the repetitive JSON keys. Keep JSON lines.
- Dropping `inverse` saves a further ~15 % compressed (400 → 338 KB) but is a semantic change: undo history across reload would then need to be recomputed by replay. Not recommended in the first step; list as an option.
- Absolute sizes are small either way: the whole 141-patch corpus is 6.5 MB today and ~0.4 MB compressed.

## 3. Append-friendliness

One zstd-19 frame per save (JSON, no `inverse`), against one frame for the whole log, no dictionary:

| Ops per save | Total | vs single frame |
|---|---|---|
| 1 | 2,465,499 | x7.29 |
| 10 | 670,244 | x1.98 |
| 50 | 421,498 | x1.25 |

Tiny frames compress badly, because each starts with no history. Appending a frame per save is only worth it with a shipped dictionary (not measured in combination) and periodic compaction (rewrite one frame when the file's frame count or waste crosses a threshold). Note that a single save rewrite of a median patch costs ~14 ms at level 19, so "rewrite on save" is also cheap: the append path is an optimisation for large logs, not a necessity.

## 4. Load speed: replay vs checkpoint

`load_bench` (release build, includes toml, migration check and `composite::validate`), corrected after profiling: warm, the whole 139-patch corpus loads in **32 ms**, a mean of **0.24 ms** per patch for full replay; the slowest is `performance-project` at 2.4 ms for 923 entries (cost scales with entry count, about 2.5 us per op). Parsing the whole `checkpoint.json` as `serde_json::Value` is a 0.05 ms lower bound. The first version of this section reported 7.5 ms mean and a 122 ms outlier (`robot-voice`, 100 entries): that was a cold file cache and a busy machine, not a hot spot (`robot-voice` loads in 0.15 ms cold, 0.08 ms warm, run alone). Verdict: replay is already fast enough that a snapshot would save well under 1 ms per patch while costing +36 % file size (log 400 KB + checkpoint 261 KB compressed separately, 545 KB together) and creating a second source of truth: **not worth it**. Compacting on save is cheap and recommended (see layout).

## 5. Container options

| Option | Append cost | Crash safety | Random access | Inspectable | Notes |
|---|---|---|---|---|---|
| zip, stored entries + zstd | Rewrite central directory at end; append entry possible | Torn tail leaves a broken directory unless rewritten via temp file | Yes, per entry | Yes (`unzip -l`, but zstd method needs 7-Zip 23+) | `zip` crate pulls many features; one entry per save is the same small-frame problem |
| tar.zst | Not appendable (whole stream) | Atomic rename only | No | Yes (`tar --zstd`) | Simple, but every save recompresses everything |
| **Multi-frame zstd file (one appended frame per save, skippable-frame headers/commits)** | One write + fsync at EOF | Per-frame checksum; torn tail detected and ignored/truncated; compaction via temp + atomic rename | Frame index in a trailing skippable frame (not needed at these sizes) | **Yes: it is a valid zstd stream, `zstd -dc x.kabl` prints the log** (add `-D kabl.dict` if a dictionary is used) | Smallest, smallest dependency surface |
| SQLite with compressed blobs | Cheap append | Excellent (WAL, transactions) | Yes (SQL) | Needs a tool, schema, and the blobs are still compressed | `rusqlite` bundles ~1 MB of C; overkill for one append-only log; binary-diff story is worse |

## 6. Recommended design: multi-frame zstd file

**Layout** (`*.kabl`, one file, little-endian):

```
[skippable frame 0x184D2A50]  "KABL" | container_version u16 | schema_version u16 | dict_id u32
[zstd frame]                  base: compacted log, JSON lines (entries seq 0..n)
[skippable frame 0x184D2A51]  commit: entry_count u32 | xxh3/crc32 of all frames so far
[zstd frame]                  appended entries for one save (JSON lines)
[skippable frame 0x184D2A51]  commit
...
```

- zstd level 19, content checksum on, **shipped trained dictionary** (about 50-110 KB in the binary), dictionary id in each frame header. Dictionary is versioned; a file always names the id it needs, old dictionaries stay in the binary.
- Log stays JSON lines with `inverse` (no semantic change). The checkpoint is dropped; `meta.toml` becomes the header frame.
- **Save:** if the patch was loaded from a `.kabl` and only appended since, append a data frame plus a commit frame and fsync. Otherwise (first save, undo past the base, or waste threshold reached, e.g. more than 16 appended frames or >25 % of bytes in frames smaller than 1 KB) write a compacted single-base file to `name.kabl.tmp`, fsync, atomic rename.
- **Load:** read, decode frames in order, stop at the first frame whose commit is missing or checksum fails (torn append), replay as today.
- **Existing folders** keep loading (`format::load(dir)`); the library recognises both. `save` writes `.kabl`.
- **CLAP:** unchanged. Host bytes are `SoundState` JSON, not a patch file. If a `.kabl` ever needs to travel through host state, expose `format::{read,write}` over `Read`/`Write` or `&[u8]` rather than a path, and keep the `MAX_STATE_BYTES` cap on the decoded size (a decompression bound).
- **Expected size:** ~400 KB for the 141-patch corpus without a dictionary, ~350 KB with it (est. from the -12 % held-out result), against 6.5 MB of JSON today: roughly 16-18x.

**What it gives up**
- Plain-text `git diff` and merge of `log.jsonl`. Mitigation: `.gitattributes` `*.kabl diff=kabl` with `textconv = zstd -dc -D …`, or a small `kabl cat` subcommand. Factory patches checked into `patches/` can stay as folders (the loader reads both) and be packed to `.kabl` at packaging time, so source diffs stay readable and the repo does not store binary blobs.
- Hand-editing with a text editor (needs `zstd -d`, then re-pack).
- A one-shot dictionary dependency: a file needs the matching dictionary to open.
- Smaller gain from brotli/xz (another 5-9 %) was rejected for speed, crate size, and because the dictionary beats it.

Rejected: zip (overlapping features, more crate surface, no better append story), tar.zst (not appendable), SQLite (too heavy, weaker inspectability), CBOR/msgpack (bigger after compression).

## 7. Dependencies

Not in `Cargo.lock` today: `zstd`, `zstd-sys`, `zip`. Already present: `flate2`, `ciborium` (transitive).

| Crate | Licence (checked in local registry) | `deny.toml` | Size cost (estimate, not measured: no dependency was added) |
|---|---|---|---|
| `zstd` 0.13 (+ `zstd-sys` 2.0, vendors libzstd 1.5.7) | MIT; libzstd is BSD-3-Clause (dual with GPL-2.0) | OK (MIT, BSD-3-Clause allowed); needs a C compiler | ~0.4-0.7 MB with compressor and trainer; far less if decoder-only |
| `ruzstd` (pure Rust) | MIT | OK | ~0.1 MB; decoder fine, but encoder only reaches fast levels, so it cannot give the level-19 ratio |
| `zip` | MIT | OK | larger, not recommended |
| `brotli`, `xz2`/`lzma-rs` | MIT/BSD, MIT/Apache | OK | 0.3-0.8 MB |
| `rusqlite` (bundled) | MIT | OK | ~1 MB |

Recommendation: `zstd` crate, with the dictionary trained offline by a script (not at runtime), so the shipped binary needs encode and decode but not the trainer (disable that feature).

## 8. Migration plan and size

1. **Container in `kabl-core`** (PR 1, ~500-700 LOC incl. tests): `format::{read_kabl, write_kabl}` over `Read`/`Write`, header/commit frames, torn-tail and checksum tests, atomic rename on compaction. Add `zstd`. Folders still load.
2. **App integration** (PR 2, ~300-400 LOC): library recognises `*.kabl` and folders, Save/Save As write `.kabl`, "Pack folder to .kabl" on open of a legacy patch, recents update.
3. **Dictionary** (PR 3, ~150 LOC + binary asset 50-110 KB): training script in `docs/format/` or `scripts/`, versioned dictionary, CI check that dictionary id is known.
4. **Tooling and migration** (PR 4, ~200 LOC): `kabl cat/pack/unpack` subcommands, `.gitattributes` textconv recipe, optionally pack factory patches at packaging time.
5. **Append path** (PR 5, ~200 LOC, optional, only if profiling shows saves matter): append frames and the compaction threshold. A median patch compacts in ~14 ms, so this could be skipped.

Total: 4 PRs required (~1.2-1.5k LOC), a 5th optional. Order matters: 1 before 2, 3 can follow 2 (without a dictionary the format is still valid at roughly -12 % size).

## 9. Open items for Kosta
- Drop `inverse` from the file (15 % smaller, loses persisted undo)? Default here: keep.
- Keep factory patches as folders in git, or switch to `.kabl` in the repo? Default here: folders in git, `.kabl` in packaging.
- Dictionary + multi-frame overhead was not measured together; verify in PR 3 before relying on append.
