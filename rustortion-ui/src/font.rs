/// Embedded font bytes (Inter Regular, SIL Open Font License).
///
/// Bundling a font guarantees text renders on all systems regardless of which
/// system fonts are installed. Without this, iced falls back through
/// cosmic-text's hardcoded list (Noto Sans, DejaVu Sans, ...) and text
/// silently disappears on systems that lack those fonts.
pub const EMBEDDED_FONT_BYTES: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

/// The [`iced::Font`] that references the embedded font by name.
pub const EMBEDDED_FONT: iced::Font = iced::Font::with_name("Inter");
