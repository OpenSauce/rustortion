# Test fixtures

## `reference_standard.nam`

A standard-architecture WaveNet NAM model, vendored from the
[`nam-rs`](https://github.com/OpenSauce/nam-rs) test fixtures
(`tests/fixtures/reference_standard.nam`).

It is used by the NAM parity test (`block_matches_per_sample_with_real_model`) and
the `chain` benchmark's NAM groups, so both run deterministically in CI without
depending on a user's personal (gitignored) `nam/` models.

### License / attribution

`nam-rs` is distributed under the MIT License (Copyright (c) 2026 Leigh). The `.nam`
weight/config layout is a derivative of the Neural Amp Modeler ecosystem
(neural-amp-modeler / NeuralAmpModelerCore, Copyright (c) 2019-2025 Steven Atkinson,
MIT). See the `nam-rs` `LICENSE` and `NOTICE` files for full terms.
