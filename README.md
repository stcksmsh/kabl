# kabl

A legible modular synthesizer, written in Rust. Cables are instruments, patches explain
themselves, modules can be built like guitar pedals.

Project brief and architecture: see the agent brief this repo was built from (owner-held, not
checked in here).

**Start here: [`docs/STATUS.md`](docs/STATUS.md)** — current state, what's real vs. stub, what's
next. Kept up to date on every commit; read it before the code if you're picking this up cold
(human or agent).

Other working documents:

- `docs/decisions.md` — every non-trivial choice and the alternative rejected, append-only.
- `docs/proposals.md` — out-of-scope ideas, parked.
- `docs/confusions.md` — beginner confusions, lesson source material.
- `docs/benchmarks.md` — measured performance history, append-only.

## Workspace

```
crates/
  core/        # op log, patch state, replay, undo, file format — no audio deps
  engine/      # graph compiler, scheduler, voice allocator, swap/crossfade, quality tiers
  modules/     # built-in modules + metadata
  cables/      # cable node impl, step patterns, probability, morph
  pedals/      # composite modules, code modules
  learn/       # lesson format, unlock state
  ui/          # egui patchbay canvas, scopes, explain overlay
  standalone/  # cpal + midir + window
  clap/        # CLAP plugin wrapper
```

## Status

See [`docs/STATUS.md`](docs/STATUS.md) for the current, detailed answer. Short version:
the engine, 9 modules, a live standalone synth (`kabl`) and a patchbay UI (`kabl-ui`) exist and play.

## Building

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

`core` has zero audio dependencies and can be built/tested standalone. `engine`'s tests enforce
real-time safety (`assert_no_alloc`) on the swap/process path.
