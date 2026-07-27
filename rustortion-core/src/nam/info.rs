//! Display-ready summary of a `.nam` file's metadata.
//!
//! [`nam_rs::NamModel::metadata_typed`] clones and re-parses the raw JSON on every
//! call — including the trainer's `training` blob, which is by far the largest thing
//! in the file. That is fine at load time and completely wrong in a GUI `view()`,
//! which runs every frame. So the fields the UI actually shows are extracted once,
//! when the model is parsed, and cached in the registry.

use nam_rs::NamModel;

/// Placeholder values TONE3000 writes into `gear_make`/`gear_model` when the
/// uploader left them blank. They are literal sentinels, not real gear names, and
/// they account for about half the models in circulation — showing them would be
/// worse than showing nothing.
const PLACEHOLDER_GEAR: &[&str] = &["tz-make", "tz-model"];

/// Placeholder TONE3000 writes into `tone_type` for "unspecified".
const PLACEHOLDER_TONE: &[&str] = &["t3k-null"];

/// Treat blank strings and known placeholder sentinels as absent.
fn meaningful(value: Option<String>, placeholders: &[&str]) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() || placeholders.contains(&trimmed.to_ascii_lowercase().as_str()) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// The subset of a model's metadata worth putting on screen, extracted once.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelInfo {
    /// Whether the capture already contains a speaker cab. `None` when the file
    /// doesn't say, or says something we can't place — never a guess.
    pub includes_cab: Option<bool>,
    /// The modelled gear, e.g. `"Marshall JMP-50"`. Make and model are joined, but
    /// deduplicated when they're identical (files commonly repeat the full name in
    /// both). `None` when both are absent or placeholders.
    pub gear: Option<String>,
    /// Character of the tone, e.g. `"overdrive"`.
    pub tone_type: Option<String>,
    /// Who captured the model, for attribution.
    pub modeled_by: Option<String>,
    /// Output loudness in LUFS. Worth surfacing because it varies by more than
    /// 20 dB across models in circulation, which is exactly the volume jump users
    /// hear when switching between them.
    pub loudness_lufs: Option<f32>,
}

impl ModelInfo {
    /// Extract the display summary from a parsed model. Called once per model at
    /// load time, never on the audio thread and never from `view()`.
    #[must_use]
    pub fn from_model(model: &NamModel) -> Self {
        let md = model.metadata_typed();
        // Read before the String fields are moved out below.
        let includes_cab = md.includes_cab();

        let make = meaningful(md.gear_make, PLACEHOLDER_GEAR);
        let model_name = meaningful(md.gear_model, PLACEHOLDER_GEAR);
        let gear = match (make, model_name) {
            // Identical values are the same name written twice, not "Make Model".
            (Some(make), Some(model)) if make.eq_ignore_ascii_case(&model) => Some(make),
            (Some(make), Some(model)) => Some(format!("{make} {model}")),
            (Some(only), None) | (None, Some(only)) => Some(only),
            (None, None) => None,
        };

        Self {
            includes_cab,
            gear,
            tone_type: meaningful(md.tone_type, PLACEHOLDER_TONE),
            modeled_by: meaningful(md.modeled_by, &[]),
            loudness_lufs: md.loudness,
        }
    }

    /// True when there is nothing at all to show.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.gear.is_none()
            && self.tone_type.is_none()
            && self.modeled_by.is_none()
            && self.loudness_lufs.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but valid `.nam` carrying the given metadata block.
    fn model_with_metadata(metadata: &str) -> NamModel {
        let json = format!(
            r#"{{
                "version": "0.5.4",
                "architecture": "WaveNet",
                "config": {{
                    "layers": [{{
                        "input_size": 1, "condition_size": 1, "channels": 1,
                        "head_size": 1, "kernel_size": 1, "dilations": [1],
                        "activation": "ReLU", "gated": false, "head_bias": false
                    }}],
                    "head": null, "head_scale": 1.0
                }},
                "weights": [1.0, 2.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0],
                "metadata": {metadata}
            }}"#
        );
        NamModel::from_json_str(&json).expect("fixture must parse")
    }

    #[test]
    fn extracts_the_displayable_fields() {
        let info = ModelInfo::from_model(&model_with_metadata(
            r#"{"gear_make": "Marshall", "gear_model": "JMP-50",
                "tone_type": "overdrive", "modeled_by": "somebody",
                "loudness": -19.4, "gear_type": "amp"}"#,
        ));
        assert_eq!(info.gear.as_deref(), Some("Marshall JMP-50"));
        assert_eq!(info.tone_type.as_deref(), Some("overdrive"));
        assert_eq!(info.modeled_by.as_deref(), Some("somebody"));
        assert_eq!(info.includes_cab, Some(false));
        assert!((info.loudness_lufs.expect("loudness") - -19.4).abs() < 1e-4);
        assert!(!info.is_empty());
    }

    /// Roughly half of all models in circulation carry these sentinels. Showing
    /// "tz-make tz-model" as the amp name would be worse than showing nothing.
    #[test]
    fn placeholder_gear_and_tone_are_dropped() {
        let info = ModelInfo::from_model(&model_with_metadata(
            r#"{"gear_make": "tz-make", "gear_model": "tz-model",
                "tone_type": "T3K-Null", "loudness": -22.0}"#,
        ));
        assert_eq!(info.gear, None);
        assert_eq!(info.tone_type, None);
        // ...but the real data alongside them still comes through.
        assert!(info.loudness_lufs.is_some());
        assert!(!info.is_empty());
    }

    #[test]
    fn identical_make_and_model_are_not_repeated() {
        let info = ModelInfo::from_model(&model_with_metadata(
            r#"{"gear_make": "Marshall JMP-50 Lead 1969 Plexi",
                "gear_model": "Marshall JMP-50 Lead 1969 Plexi"}"#,
        ));
        assert_eq!(
            info.gear.as_deref(),
            Some("Marshall JMP-50 Lead 1969 Plexi")
        );
    }

    #[test]
    fn one_sided_gear_still_shows() {
        let info = ModelInfo::from_model(&model_with_metadata(
            r#"{"gear_make": "Fender", "gear_model": "tz-model"}"#,
        ));
        assert_eq!(info.gear.as_deref(), Some("Fender"));
    }

    #[test]
    fn blank_strings_count_as_absent() {
        let info = ModelInfo::from_model(&model_with_metadata(
            r#"{"gear_make": "  ", "gear_model": "", "modeled_by": " "}"#,
        ));
        assert_eq!(info.gear, None);
        assert_eq!(info.modeled_by, None);
        assert!(info.is_empty());
    }

    #[test]
    fn absent_metadata_yields_an_empty_summary() {
        let info = ModelInfo::from_model(&model_with_metadata("null"));
        assert_eq!(info, ModelInfo::default());
        assert!(info.is_empty());
        assert_eq!(info.includes_cab, None);
    }

    #[test]
    fn cab_inclusive_gear_is_flagged() {
        for gear in ["amp_cab", "full-rig", "cab"] {
            let info = ModelInfo::from_model(&model_with_metadata(&format!(
                r#"{{"gear_type": "{gear}"}}"#
            )));
            assert_eq!(info.includes_cab, Some(true), "{gear}");
        }
    }
}
