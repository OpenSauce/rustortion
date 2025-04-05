# Rustortion 🎸

A low-latency guitar amp pass-through with gain control, built in Rust using JACK (via PipeWire).

## Requirements

- Linux with PipeWire (with JACK support enabled)
- `libjack` and JACK tools installed:  
  `sudo apt install libjack-jackd2-dev jackd2 jack-tools`
- Rust: [https://rust-lang.org/tools/install](https://rust-lang.org/tools/install)

## Building

`cargo build --release`

## Running

Run with default gain (1.5x):  
`./target/release/rustortion`

Or specify a custom gain:  
`./target/release/rustortion 2.0`

💡 Gain values around `1.0`–`2.0` work well. Values above `1.0` will boost volume and distortion.

## Notes

- Automatically connects to `system:capture_1` (e.g. guitar input) and `system:playback_1` / `system:playback_2` (speakers/headphones)
- Audio is duplicated to both left and right channels
- Latency is minimized via PipeWire’s `PIPEWIRE_LATENCY=64/48000` setting

## TODO

- Add real distortion effects (soft/hard clipping)
- Real-time gain control
- Cabinet IR loading
