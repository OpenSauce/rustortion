## [0.1.7] - 2026-02-21

### 🚀 Features

- Add chinese (zh-CN) localization (#178)
- Add multi-band saturation (#181)
- Add pitch shifting (#189)

### 🐛 Bug Fixes

- Convert JACK sample_rate from u32 to usize (#184)
- Correct multiband saturator crossover topology and bound waveshaper (#185)
- Correct tonestack mid-band extraction and constructor clamping (#186)
- Bound poweramp saturation, fix crossover distortion, and precompute sag (#187)
- Clamp filter cutoff minimum (#188)
- Reduce latency on pitch shifting fft (#190)

### 🚜 Refactor

- Rename sim module to amp and cleanup logging (#176)

### ⚙️ Miscellaneous Tasks

- Add chinese zh-cn readme (#180)
## [0.1.6] - 2026-01-10

### 🚀 Features

- Add midi controller (#165)
- Dynamic filter range in GUI (#168)
- Update to Iced 0.14 (#169)
- Add bass guitar IR and preset (#170)
- Default to FIR only for impulse response convolution (#173)

### 🐛 Bug Fixes

- Hardcoded sample rate in preamp (#172)

### 🚜 Refactor

- Move tuner to its own module (#166)

### ⚙️ Miscellaneous Tasks

- Remove unused filter stages and field (#167)
## [0.1.5] - 2025-12-28

### 🚀 Features

- Generate the weakest changelog known to mankind (#138)
- Improve settings layout (#139)
- Save bypassing ir in settings (#141)
- Add Science Amplification impulse responses (#150)
- Add petrucci preset (#151)
- Save and load ir_gain and last selected preset (#152)
- Add process_block to stage trait (#157)

### 🐛 Bug Fixes

- Remove problematic IR and add safeguard (#148)

### 📚 Documentation

- Add pipewire cornercase workaround (#146)

### ⚙️ Miscellaneous Tasks

- Remove un-used setting (#140)
- Fix spacing/padding on main page (#142)
- *(screenshot)* Update screenshot (#153)
- Cleanup process_block (#158)
- Update dependencies (#160)
## [0.1.0] - 2025-11-16

### 🚀 Features

- Add multi stage
