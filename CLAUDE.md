# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustortion is a real-time guitar/bass amp simulator built in Rust. It runs as a standalone JACK app and as a VST3/CLAP plugin. The GUI is shared between both targets via the `rustortion-ui` crate.

## Workspace Crates

- **`rustortion-core`** — DSP engine, amp stages, IR cabinet, preset management. No GUI dependencies.
- **`rustortion-ui`** — Shared GUI: stages, components, messages, handlers, i18n, `SharedApp<ParamBackend>`. Uses `iced = "0.14"`.
- **`rustortion-standalone`** — Standalone JACK app. Thin shell wrapping `SharedApp<StandaloneBackend>` with MIDI, tuner, settings, recording.
- **`rustortion-plugin`** — VST3/CLAP plugin via nih-plug. Editor uses `iced_baseview` + `SharedApp<PluginBackend>`.
- **`xtask`** — Build automation.

## Build & Development Commands

```bash
# Build and run standalone (requires JACK/PipeWire)
cargo run --release

# Build plugin
cargo build -p rustortion-plugin --release

# Lint (formatting + clippy) — this is what CI runs
make lint

# Run tests
make test                    # all tests
cargo test test_name         # single test

# Benchmarks
make bench

# Coverage (requires cargo-tarpaulin)
make cover
```

**System dependencies** (must be installed before building):
```bash
sudo apt-get install libjack-jackd2-dev libasound2-dev pkg-config
```

**Clippy flags** used in CI: `-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery`
(`lib.rs` has `#![allow(...)]` overrides for specific pedantic/nursery lints)

**Dev profile** uses `opt-level = 1` because IR cabinet processing is too slow in pure debug mode.

## Architecture

### Shared GUI Pattern

Both standalone and plugin use `SharedApp<B: ParamBackend>` from `rustortion-ui`:

```
rustortion-standalone                    rustortion-plugin
  AmplifierApp                             PluginApp (iced_baseview::Application)
    └─ SharedApp<StandaloneBackend>          └─ SharedApp<PluginBackend>
         └─ StandaloneBackend                     └─ PluginBackend
              └─ Manager/Engine (JACK)                  └─ EngineHandle + GuiContext
```

`ParamBackend` trait (`rustortion-ui/src/backend.rs`) abstracts engine communication. `Capabilities` struct controls which UI sections render (e.g. plugin hides tuner, MIDI config, recording, settings).

### Audio Signal Flow

```
Input → [Tuner bypass] → Input Filters (HP/LP) → [Upsample] → Amp Chain (stages) → [Downsample] → Pitch Shifter → IR Cabinet → Peak Meter → Recorder → Output
```

### Key Modules

#### rustortion-core
- **`src/amp/chain.rs`** — Ordered list of processing stages.
- **`src/amp/stages/`** — 10 registered DSP stages: preamp, compressor, noise_gate, tonestack, poweramp, multiband_saturator, level, delay, reverb, eq. Plus utilities: `clipper`, `filter`, `common`.
- **`src/audio/engine.rs`** — Core audio processing loop. Controlled via crossbeam channels.
- **`src/ir/`** — IR cabinet, convolver (FIR/FFT), loader.
- **`src/preset/`** — Preset save/load/delete, `StageConfig` enum, `InputFilterConfig`.

#### rustortion-ui
- **`src/app.rs`** — `SharedApp<B>` — shared state, update(), view(), subscription().
- **`src/backend.rs`** — `ParamBackend` trait, `Capabilities`, `ExternalEvent`.
- **`src/stages/mod.rs`** — `gui_stage_registry!` macro, `ParamUpdate`, all 10 stage view modules.
- **`src/components/`** — Reusable UI components: widgets, dialogs, preset_bar, peak_meter, ir_cabinet_control, minimap, etc.
- **`src/handlers/`** — Portable handlers: preset, hotkey.
- **`src/messages/`** — Message enums for Iced event-driven updates.
- **`src/i18n/`** — `tr!()` macro, EN + ZH_CN locales.
- **`src/tabs.rs`** — Tab navigation: Amp, Effects, Cabinet, IO.

#### rustortion-standalone
- **`src/gui/app.rs`** — `AmplifierApp` wrapping `SharedApp<StandaloneBackend>` + standalone handlers (MIDI, tuner, settings, recording).
- **`src/backend.rs`** — `StandaloneBackend` implementing `ParamBackend` via `Manager`/`Engine`.
- **`src/audio/`** — JACK client, Manager, ports.
- **`src/gui/handlers/`** — Standalone-only: midi, tuner, settings.
- **`src/gui/components/dialogs/`** — Standalone-only dialogs: midi, settings, tuner.

#### rustortion-plugin
- **`src/lib.rs`** — nih-plug `Plugin` impl, audio processing, initialization.
- **`src/editor.rs`** — `PluginEditor` (nih-plug `Editor` trait) + `PluginApp` (iced_baseview `Application`).
- **`src/backend.rs`** — `PluginBackend` implementing `ParamBackend` via `EngineHandle` + `GuiContext`.
- **`src/params.rs`** — Full nih-plug parameter set: global params + 8 slots × 10 stage types.

### Stage Registration (`rustortion-ui/src/stages/mod.rs`)

The `gui_stage_registry!` macro generates `StageType`, `StageConfig`, and `StageMessage` enums plus all boilerplate. Adding a new stage requires:
1. Add one line to the macro invocation
2. Create `rustortion-ui/src/stages/new_stage.rs` with config, message, and view implementations
3. Create `rustortion-core/src/amp/stages/new_stage.rs` implementing the `Stage` trait
4. Add i18n keys to EN and ZH_CN in `rustortion-ui/src/i18n/mod.rs`
5. Add slot params to `rustortion-plugin/src/params.rs`

### Thread Model

The JACK process callback (standalone) or nih-plug `process()` (plugin) runs on a real-time thread. The GUI communicates with the engine via crossbeam channels. Shared state (tuner data, peak meter) uses `ArcSwap` for lock-free reads.

## Common Pitfalls

- **JACK/PipeWire must be running** before `cargo run --release`. If JACK is not available the app will panic on startup.
- **Dev profile uses `opt-level = 1`** — benchmarks and performance comparisons must use `--release`.
- **The `gui_stage_registry!` macro** in `rustortion-ui/src/stages/mod.rs` generates boilerplate. Do not hand-write — add one line to the macro invocation instead.
- **Preset JSON format** — each preset is a JSON file in `~/.config/rustortion/presets/`. Structure: `{ "name": "...", "stages": [...], "ir_name": "...", "ir_gain": N, "pitch_shift_semitones": N, "input_filters": {...} }`.
- **IR files** are in `impulse_responses/` and `~/.config/rustortion/impulse_responses/`. Loading is async (off RT thread).
- **Clippy is strict** — CI runs `-D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery`.
- **iced_baseview** is a fork at `github.com/OpenSauce/iced_baseview`, upgraded to iced 0.14 crates.io.

## Conventions

- Rust edition 2024
- Conventional commits: `feat:`, `fix:`, `refactor:`, `chore:`, etc.
- Changelog generated via `git-cliff`
- Standalone entry point: `rustortion-standalone/src/bin/gui.rs`
- Releases via `cargo-dist` (`.github/workflows/release.yml`, `dist-workspace.toml`)
