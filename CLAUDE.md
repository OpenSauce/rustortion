# CLAUDE.md

Rustortion is a real-time guitar/bass amp simulator in Rust. It ships as a standalone JACK app
and as a VST3/CLAP plugin; both drive the same GUI via `SharedApp<B: ParamBackend>`.

Workspace: `rustortion-core` (DSP, no GUI deps) · `rustortion-ui` (shared iced 0.14 GUI) ·
`rustortion-standalone` (JACK) · `rustortion-plugin` (nih-plug) · `xtask`.

## Commands

```bash
cargo run --release              # standalone (needs JACK/PipeWire; exits 1 with a clear error if absent)
make lint                        # fmt + clippy — exactly what CI runs
make test                        # cargo test --workspace --all-targets --all-features
make plugin                      # cargo xtask bundle rustortion-plugin --release
make plugin-install              # copy bundle to ~/.clap and ~/.vst3
make bench / make cover
```

CI clippy: `-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery`. Each `lib.rs`
carries its own `#![allow(...)]` block (they have drifted — see REV-20 in `claude/tasks.md`).

System deps: `libjack-jackd2-dev libasound2-dev pkg-config`.

Toolchain is pinned in `rust-toolchain.toml` (1.95.0). CI runs `@stable`, so an unpinned bump can
turn a release build red *after* the tag is pushed — bump it in its own PR.

## Releasing

Tag push → `release.yml` (cargo-dist; builds the standalone for x86_64 + aarch64 Linux and calls
`gh release create`) → `release: published` fires `plugin-release.yml`, which bundles the plugin
natively on Linux x86_64, Linux aarch64 and Windows x86_64 and uploads the zips. The `published`
trigger is load-bearing: a same-tag-push trigger races dist into a 404 on every upload.
`release.yml` is dist-generated — never hand-edit it. Version lives in the four crate
`Cargo.toml`s (kept in lockstep; `xtask` stays 0.1.0 and is `dist = false`). Changelog is
`make changelog TAG=v0.3.0` — the `TAG` is required, since without it git-cliff files the
commits you are releasing under `[unreleased]` instead of the version being cut.

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

1. `rustortion-core/src/amp/stages/new_stage.rs` — implement the `Stage` trait, including the
   required `reset()` (zero every buffer/index in place; never allocate — hosts call it from the
   RT thread)
2. `rustortion-core/src/preset/stage_config.rs` — add the variant to every match (compiler will list them)
3. `rustortion-ui/src/stages/new_stage.rs` — config, message, `apply()`, `view()`
4. One line in the `gui_stage_registry!` invocation
5. i18n keys in **both** EN and ZH_CN (`rustortion-ui/src/i18n/mod.rs`, `tr!()` macro)

Nothing to add in `rustortion-plugin/src/params.rs` — stage parameters are not host-exposed
(see Pitfalls).

## Pitfalls

- **Preset files** — one JSON per preset in `preset_dir`, which defaults to `./presets` (CWD-relative,
  and the *shipped* `presets/` is that live writable directory). `~/.config/rustortion/` holds
  `settings.json`, not presets. Filenames are percent-encoded (`[A-Za-z0-9-_]` kept verbatim), but a
  preset is identified by the `name` field *inside* the JSON — `Manager` remembers the path each
  preset was loaded from so saves/deletes reuse legacy filenames, and `path_for` refuses any
  candidate another preset already occupies (falling back to a digest-suffixed stem). Saves are
  still not atomic (`fs::write`) and save/delete failures are only logged, never surfaced in the UI
  (REV-7 remainder).
- **NAM models** (`.nam`, WaveNet + LSTM via `nam-rs`) load from a user folder with rescan, into a
  process-global registry; stages resolve them **by name**. No rfd file-picker — rfd/gtk3 breaks CI.
- **IR files** live in `impulse_responses/` and `~/.config/rustortion/impulse_responses/`; loading
  is async, off the RT thread. Keep convolver type configurable — FIR beat FFT on the Pi in real
  testing, and the TwoStage tail math is numerically wrong until REV-2 lands.
- **iced_baseview** is a fork at `github.com/OpenSauce/iced_baseview`, pinned to tag `v0.1.0`.
  Moving it means tagging a release on the fork first — don't repoint it at a bare `rev`.
- **The plugin exposes 9 global params only** (`params.rs`, ~117 lines): output level, IR gain/bypass,
  pitch shift, HP/LP enable+cutoff, preset index. The nested per-slot param arrays were deleted in
  REV-4 — they were never read by `process()`. Stage settings travel through `chain_state` (preset
  JSON in the host's state blob), so they persist and recall correctly but are **not host-automatable**.
  Exposing them properly is v0.4.0+ work.

## Conventions

Rust edition 2024 · conventional commits · changelog via `git-cliff` · releases via `cargo-dist`.
Working notes (tasks, roadmap, product, DSP context) are in `./claude/` — gitignored.
