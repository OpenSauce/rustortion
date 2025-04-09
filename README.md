# Rustortion 🎸

A basic low-latency guitar amp simulator, built in Rust using JACK (via PipeWire).

## Requirements

- Linux with PipeWire (with JACK support enabled)
- `libjack` and JACK tools installed:  
  `sudo apt install libjack-jackd2-dev jackd2 jack-tools`
- Rust: [https://rust-lang.org/tools/install](https://rust-lang.org/tools/install)

## Building

`cargo build --release`

## Running

Pass the path to the preset file as an argument:
`cargo run -- --preset-path presets/metal.json`

Use the recording flag to save the output to a file:  
`cargo run -- --recording`

Recordings are saved in the /recordings directory with a timestamp in the filename.

## TODO

- Add real distortion effects (soft/hard clipping)
- Real-time gain control
- Cabinet IR loading
