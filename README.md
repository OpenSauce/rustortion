# Rustortion

A guitar amp simulator built in Rust using JACK.

## Features

- Low-latency audio processing with oversampling
- Multiple amp simulation stages (preamp, compressor, tone stack, power amp, etc.)
- Impulse response (IR) cabinet simulation
- Preset management system
- Real-time recording capability
- GUI using [Iced](https://github.com/iced-rs/iced)

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


## Impulse Responses

This project uses impulse responses licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).  
Original source: [Open AIR Library](https://www.openair.hosted.york.ac.uk/)
