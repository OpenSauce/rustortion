// Enable pedantic lints globally, matching the rest of the workspace.
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap
)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
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
#![allow(clippy::redundant_pub_crate, clippy::significant_drop_tightening)]

pub mod app;
pub mod backend;
pub mod components;
pub mod handlers;
pub mod hotkey;
pub mod i18n;
pub mod messages;
pub mod stages;
pub mod tabs;
