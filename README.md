# Rustortion

A guitar amp simulator built in Rust using JACK.

## Screenshot

![Rustortion](screenshots/rustortion.png)

## Features

- Low-latency audio processing with oversampling
- Multiple amp simulation stages (preamp, compressor, tone stack, power amp, etc.)
- Impulse response (IR) cabinet simulation
- Save and load presets
- Real-time recording capability
- Built-in tuner
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

## Contributing

This is an experimental project. Feel free to open issues or submit pull requests.

## License

This project is provided under the **MIT License**.
Rustortion is under active development and should be used at your own risk.

### Impulse Responses

This project includes freely licensed impulse responses from [freesound.org](https://freesound.org/):

- [Multiple Cabinets – Jesterdyne](https://freesound.org/people/jesterdyne/)
- [Harley Benton 4x12 – Vihaleipa](https://freesound.org/people/Vihaleipa/sounds/269662/)
- [Bristol Mix – Mansardian](https://freesound.org/people/mansardian/sounds/648392/)
- [Brown Cab – Tosha73](https://freesound.org/people/tosha73/sounds/507167/)
