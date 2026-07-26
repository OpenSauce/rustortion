## [0.3.0] - 2026-07-26

### 🚀 Features

- Add per-stage bypass toggle (#220)
- Add nih-plug CLAP/VST3 plugin with GUI and preset support (#224)
- Shared GUI via rustortion-ui with iced_baseview plugin editor (#225)
- Assert no allocations on the RT audio path (#238)
- Add NAM neural amp modeler stage (#243)
- *(nam)* Bypass model on sample-rate mismatch and surface it in the UI (#244)
- *(nam)* Folder management — editable dir, rescan, models-folder on stage card (#245)
- *(nam)* Update to nam-rs 0.2.0 and support LSTM models (#248)
- *(stages)* Add tremolo effect with sine→square killswitch shape (#258)

### 🐛 Bug Fixes

- Move IR loading off RT thread and drain engine messages (#213) (#215)
- Forward parameter changes to live stages, eliminate rebuild clicks (#217)
- Correct power amp sag, push-pull symmetry, and add sag_release (#218)
- *(dsp)* Tube bug fixes (#223)
- Shared oversampling control with race condition fixes (#227)
- Bundle and load own font (#231)
- *(ui)* Suffix slider step literals to satisfy float_literal_f32_fallback (#265)
- *(tremolo)* Perceptual shape curve so the whole knob morphs (#259)
- *(plugin)* Make the plugin honest for the v0.3.0 release (#268)
- *(plugin)* Resolve the engine handle per call so the GUI survives a reload (#272)
- *(preset)* Stop non-ASCII preset names from colliding on disk (#275)
- *(standalone)* Report a missing JACK server instead of panicking (#277)
- *(plugin)* Flush all DSP state on host transport reset (PLUG-2) (#276)
- *(plugin)* Restore host state across a deactivate/reactivate cycle (#279)

### 📚 Documentation

- Trim CLAUDE.md and correct stage count and macro claims (#267)

### ⚡ Performance

- Per-block chain processing + batched NAM process_buffer (2.8×) (#249)
- *(rt)* Eliminate permit_alloc sites on the RT audio path (#239) (#254)
- *(rt)* Enable FTZ/DAZ on the audio thread and clean up denormal handling (#269)

### 🧪 Testing

- Add tests for preamp, compressor, tonestack, and noise gate stages (#221)
- *(nam)* Guard model-name recall + document deferred automation (#246)

### ⚙️ Miscellaneous Tasks

- Edge case fixes and code cleanup (#222)
- Fix linting issues (#235)
- *(nam)* Update nam-rs to 0.3.0 from crates.io (#256)
- *(ci)* Pin the Rust toolchain to 1.95.0 (#270)
- *(plugin)* Build the plugin in CI and attach bundles to releases (#271)
- *(plugin)* Build the plugin on aarch64 Linux and Windows (#274)
- Delete the Korg D888 reverb IRs (#278)
## [0.2.0] - 2026-03-08

### 🚀 Features

- Display number of x-runs and show CPU usage (#195)
- Add hotkeys, allow active preset to be changed by keypress (#196)
- Add collapsible stage cards with per-stage and global toggle (#201)
- Add delay stage (#203)
- Persist per-preset stage collapse state (#204)
- Add reverb stage (#206)
- UI overhaul — input filters, tabs, minimap, dialog standardization (#207)
- Add 16-band graphic EQ stage (#209)
- Add 12AX7 triode clipper and inter-stage filtering (#211)

### 🐛 Bug Fixes

- Increase fft size in pitch shifter to improve sound quality (#194)
- Reset selected stage type when switching tabs (#210)

### 🚜 Refactor

- Move components from app to handlers (#197)
- Extract DSP utilities, remove dead code, fix RT safety and minor issues (#198)
- Consolidate per-stage GUI files into src/gui/stages/ (#199)
- Deduplicate preset handler and add stage_registry! macro (#200)
- Extract shared UI constants, colors, and dialog helpers (#208)

### ⚙️ Miscellaneous Tasks

- Enable clippy pedantic+nursery lints, fix select categories (#202)
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
