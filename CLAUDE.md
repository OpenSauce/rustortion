# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustortion is a guitar amplifier simulator built in Rust. It provides real-time, low-latency audio processing via JACK with a GUI built using Iced. The application simulates various stages of a physical guitar amplifier and includes impulse response (IR) cabinet simulation.

**Requirements**: Linux with PipeWire/JACK support, Rust toolchain

## Common Commands

```bash
# Build and run
cargo run --release
pw-jack cargo run --release    # If PipeWire needs explicit JACK

# Testing
cargo test --all-targets --all-features

# Linting
cargo fmt -- --check           # Check formatting
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all

# Full lint + test
make all                       # Runs: fmt, clippy, test

# Benchmarks
cargo bench
```

## Architecture

### Module Structure

- **amp/** - Amplifier simulation stages
  - `chain.rs` - `AmplifierChain` processes stages sequentially
  - `stages/` - Individual DSP stages (preamp, compressor, tonestack, poweramp, noise_gate, filter, clipper, level)

- **audio/** - Audio I/O and DSP engine
  - `engine.rs` - Core audio processing, receives messages via crossbeam channels
  - `jack.rs` - JACK audio server integration (`ProcessHandler`)
  - `samplers.rs` - Oversampling (1x-16x) for nonlinear processing

- **ir/** - Impulse response cabinet simulation
  - `cabinet.rs` - IR processor
  - `convolver/` - FFT and FIR convolution implementations

- **gui/** - Iced-based user interface
  - `app.rs` - Main application state
  - `components/` - UI components including per-stage controls
  - `handlers/` - Event handlers

- **preset/** - JSON-based preset management
- **settings/** - Application settings (XDG config)
- **tuner/** - Built-in chromatic tuner
- **midi/** - MIDI controller support
- **i18n/** - Internationalization (en, zh-CN)

### Data Flow

```
JACK Audio → ProcessHandler → Engine → AmplifierChain → IrCabinet → JACK Output
                                ↓
                            Tuner (when enabled, mutes output)
```

### Key Design Patterns

**Stage Trait** (`src/amp/stages/mod.rs`): All amp processing stages implement:
```rust
trait Stage: Send + Sync + 'static {
    fn process(&mut self, input: f32) -> f32;
    fn process_block(&mut self, input: &mut [f32]);
    fn set_parameter(&mut self, name: &str, value: f32) -> Result<(), &'static str>;
    fn get_parameter(&self, name: &str) -> Result<f32, &'static str>;
}
```

**Engine Handle Pattern**: The GUI communicates with the real-time audio thread via `EngineHandle` using crossbeam bounded channels. Messages are defined in `EngineMessage` enum.

**Threading Model**:
- JACK real-time thread runs `ProcessHandler`
- GUI thread runs Iced event loop
- Communication is non-blocking via crossbeam channels

## Build Dependencies

System packages needed: `libjack-jackd2-dev libasound2-dev pkg-config`
