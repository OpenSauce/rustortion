// Enable pedantic lints globally, then allow the noisy ones we're not fixing.
#![warn(clippy::pedantic, clippy::nursery)]
// --- Intentionally allowed ---
// ~140 instances: not a public API, adding #[must_use] everywhere is noise
#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]
// DSP variable names (treble_lp vs treble_hp, etc.) are intentionally similar
#![allow(clippy::similar_names)]
// GUI handlers are inherently long
#![allow(clippy::too_many_lines)]
// Audio code performs intentional casts
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap
)]
// Not a public API — no need for doc sections
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
// Style/pedantic lints that add noise without value here
#![allow(
    clippy::module_name_repetitions,
    clippy::items_after_statements,
    clippy::option_if_let_else,
    clippy::doc_markdown,
    clippy::float_cmp,
    clippy::match_same_arms,
    clippy::struct_field_names,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::if_not_else,
    clippy::match_wildcard_for_single_variants,
    clippy::single_match_else,
    clippy::unnested_or_patterns,
    clippy::trivially_copy_pass_by_ref
)]
// Nursery lints that are too noisy or not applicable
#![allow(clippy::redundant_pub_crate, clippy::significant_drop_tightening)]

pub mod amp;
pub mod audio;
pub mod gui;
pub mod hotkey;
pub mod i18n;
pub mod ir;
pub mod metronome;
pub mod midi;
pub mod preset;
pub mod settings;
pub mod tuner;
