# Rustortion

A guitar amp simulator built in Rust using JACK.

## Features

- Low-latency audio processing with oversampling
- Multiple amp simulation stages (preamp, compressor, tone stack, power amp, etc.)
- Impulse response (IR) cabinet simulation
- Preset management system
- Real-time recording capability
- Cross-platform GUI using Iced

## Requirements

- **Linux** with PipeWire (JACK support enabled)
- **Rust** toolchain: [Install Rust](https://rustup.rs/)
- **System dependencies:**
  ```bash
  sudo apt-get install libjack-jackd2-dev pkg-config
  ```

## Running

```bash
cargo run --release
```

## License

This project is under development and should be used at your own risk.

## Contributing

This is an experimental project. Feel free to open issues or submit pull requests.
