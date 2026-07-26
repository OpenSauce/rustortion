# CLAUDE.md

Rustortion is a real-time guitar/bass amp simulator in Rust. It ships as a standalone JACK app
and as a VST3/CLAP plugin; both drive the same GUI via `SharedApp<B: ParamBackend>`.

Workspace: `rustortion-core` (DSP, no GUI deps) · `rustortion-ui` (shared iced 0.14 GUI) ·
`rustortion-standalone` (JACK) · `rustortion-plugin` (nih-plug) · `xtask`.

## Commands

```bash
cargo run --release              # standalone (JACK/PipeWire must be running or it panics)
make lint                        # fmt + clippy — exactly what CI runs
make test                        # cargo test --workspace --all-targets --all-features
make plugin                      # cargo xtask bundle rustortion-plugin --release
make plugin-install              # copy bundle to ~/.clap and ~/.vst3
make bench / make cover
```

CI clippy: `-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery`. Each `lib.rs`
carries its own `#![allow(...)]` block (they have drifted — see REV-20 in `claude/tasks.md`).

System deps: `libjack-jackd2-dev libasound2-dev pkg-config`.

Dev profile uses `opt-level = 1` — IR convolution is unusably slow at `opt-level = 0`. Any
performance claim must come from `--release`.

## Signal flow

```
Input → [Tuner bypass] → Input Filters (HP/LP) → [Upsample] → Amp Chain → [Downsample]
      → Pitch Shifter → IR Cabinet → Peak Meter → Recorder → Output
```

The engine drives the chain per-block. Stages may override `Stage::process_block` (NAM does,
via nam-rs's batched `process_buffer`); the default impl loops per sample.

RT thread = JACK process callback (standalone) or nih-plug `process()` (plugin). GUI → engine is
crossbeam channels; engine → GUI is `ArcSwap` (tuner, peak meter). Nothing allocates, locks, or
does I/O on the RT path — `rustortion-core/tests/no_alloc.rs` enforces this in CI.

## Adding a stage

There are **12** registered stages. The `gui_stage_registry!` macro in
`rustortion-ui/src/stages/mod.rs` generates only `StageMessage` and three dispatch fns —
`StageType`/`StageConfig` live in core and are **hand-maintained across ~9 match sites** in
`rustortion-core/src/preset/stage_config.rs`. Adding a stage means:

1. `rustortion-core/src/amp/stages/new_stage.rs` — implement the `Stage` trait
2. `rustortion-core/src/preset/stage_config.rs` — add the variant to every match (compiler will list them)
3. `rustortion-ui/src/stages/new_stage.rs` — config, message, `apply()`, `view()`
4. One line in the `gui_stage_registry!` invocation
5. i18n keys in **both** EN and ZH_CN (`rustortion-ui/src/i18n/mod.rs`, `tr!()` macro)
6. Slot params in `rustortion-plugin/src/params.rs`

## Pitfalls

- **Preset files** — one JSON per preset in `~/.config/rustortion/presets/`. Filenames are
  percent-encoded (`[A-Za-z0-9-_]` kept verbatim), but a preset is identified by the `name` field
  *inside* the JSON — `Manager` remembers the path each preset was loaded from so saves/deletes
  reuse legacy filenames. Saves are still not atomic (`fs::write`) and save/delete failures are
  only logged, never surfaced in the UI (REV-7 remainder).
- **NAM models** (`.nam`, WaveNet + LSTM via `nam-rs`) load from a user folder with rescan, into a
  process-global registry; stages resolve them **by name**. No rfd file-picker — rfd/gtk3 breaks CI.
- **IR files** live in `impulse_responses/` and `~/.config/rustortion/impulse_responses/`; loading
  is async, off the RT thread. Keep convolver type configurable — FIR beat FFT on the Pi in real
  testing, and the TwoStage tail math is numerically wrong until REV-2 lands.
- **iced_baseview** is a fork at `github.com/OpenSauce/iced_baseview` (unpinned git dep).
- **Plugin per-slot params are not read by `process()`** — host automation of stage params is dead
  until REF-Q3/REV-4 is resolved.

## Conventions

Rust edition 2024 · conventional commits · changelog via `git-cliff` · releases via `cargo-dist`.
Working notes (tasks, roadmap, product, DSP context) are in `./claude/` — gitignored.
