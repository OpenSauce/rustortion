//! Display-ready summary of a `.nam` file's metadata.
//!
//! [`nam_rs::NamModel::metadata_typed`] clones and re-parses the raw JSON on every
//! call — including the trainer's `training` blob, which is by far the largest thing
//! in the file. That is fine at load time and completely wrong in a GUI `view()`,
//! which runs every frame. So the fields the UI actually shows are extracted once,
//! when the model is parsed, and cached in the registry.

use nam_rs::NamModel;

/// Sentinels exporters write into a descriptive field the uploader left blank.
/// `T3K-Null` is TONE3000's literal "nothing entered" marker (T3K is their own
/// abbreviation, as in the `t3k` licence value); `tz-make`/`tz-model` are the same
/// idea from an older exporter. They are strings that look like data, and they
/// account for about half the models in circulation — displaying them verbatim
/// would be worse than showing nothing.
const PLACEHOLDERS: &[&str] = &["tz-make", "tz-model", "t3k-null"];

/// Treat blank strings and known placeholder sentinels as absent.
fn meaningful(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() || PLACEHOLDERS.contains(&trimmed.to_ascii_lowercase().as_str()) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Whether a model's *name* claims a full signal chain.
///
/// Only consulted when the metadata gives us nothing to go on — see
/// [`ModelInfo::cab_from_name`]. Deliberately narrow: it matches "full rig" (however
/// it's punctuated) and a standalone "cab"/"cabs"/"cabinet" token, and nothing else.
/// A looser match would start reading cabs into names that merely happen to contain
/// the letters, and a false "this has a cab" costs the user their IR.
fn name_suggests_cab(name: &str) -> bool {
    let normalized: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if matches!(c, '-' | '_' | '.' | '/') {
                ' '
            } else {
                c
            }
        })
        .collect();

    if normalized.contains("full rig") || normalized.contains("fullrig") {
        return true;
    }

    normalized.split_whitespace().any(|word| {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        matches!(word, "cab" | "cabs" | "cabinet")
    })
}

/// The subset of a model's metadata worth putting on screen, extracted once.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelInfo {
    /// Whether the capture already contains a speaker cab. `None` when the file
    /// doesn't say, or says something we can't place — never a guess.
    pub includes_cab: Option<bool>,
    /// True when [`Self::includes_cab`] was read off the model's *name* rather than
    /// its `gear_type`, because the uploader filled in no descriptive metadata at
    /// all. Such files habitually leave `gear_type` at its default `amp` while
    /// recording the truth in the name (`…-FullRig` with every other field a
    /// placeholder). Surfaced so the UI can mark the conclusion as inferred — a
    /// guess must never be displayed as though it were a fact.
    pub cab_from_name: bool,
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
    ///
    /// `fallback_name` is the model's display name (its file stem), used only when
    /// the metadata carries no usable `name` of its own.
    #[must_use]
    pub fn from_model(model: &NamModel, fallback_name: &str) -> Self {
        let md = model.metadata_typed();
        // Read before the String fields are moved out below.
        let metadata_cab = md.includes_cab();

        let make = meaningful(md.gear_make);
        let model_name = meaningful(md.gear_model);
        let gear = match (make, model_name) {
            // Identical values are the same name written twice, not "Make Model".
            (Some(make), Some(model)) if make.eq_ignore_ascii_case(&model) => Some(make),
            (Some(make), Some(model)) => Some(format!("{make} {model}")),
            (Some(only), None) | (None, Some(only)) => Some(only),
            (None, None) => None,
        };
        let tone_type = meaningful(md.tone_type);
        let name = meaningful(md.name);

        // `gear_type` is authoritative when the uploader filled anything in. When
        // they filled in *nothing* — every descriptive field a placeholder — the
        // `amp` we're left with is a form default rather than a statement, so the
        // name is better evidence. Only ever upgrades to "has a cab": a name that
        // doesn't mention one tells us nothing either way.
        let descriptive_unfilled = gear.is_none() && tone_type.is_none();
        let (includes_cab, cab_from_name) = match metadata_cab {
            Some(true) => (Some(true), false),
            other
                if (other.is_none() || descriptive_unfilled)
                    && name_suggests_cab(name.as_deref().unwrap_or(fallback_name)) =>
            {
                (Some(true), true)
            }
            other => (other, false),
        };

        Self {
            includes_cab,
            cab_from_name,
            gear,
            tone_type,
            modeled_by: meaningful(md.modeled_by),
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
        let info = ModelInfo::from_model(
            &model_with_metadata(
                r#"{"gear_make": "Marshall", "gear_model": "JMP-50",
                "tone_type": "overdrive", "modeled_by": "somebody",
                "loudness": -19.4, "gear_type": "amp"}"#,
            ),
            "fixture",
        );
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
        let info = ModelInfo::from_model(
            &model_with_metadata(
                r#"{"gear_make": "tz-make", "gear_model": "tz-model",
                "tone_type": "T3K-Null", "loudness": -22.0}"#,
            ),
            "fixture",
        );
        assert_eq!(info.gear, None);
        assert_eq!(info.tone_type, None);
        // ...but the real data alongside them still comes through.
        assert!(info.loudness_lufs.is_some());
        assert!(!info.is_empty());
    }

    #[test]
    fn identical_make_and_model_are_not_repeated() {
        let info = ModelInfo::from_model(
            &model_with_metadata(
                r#"{"gear_make": "Marshall JMP-50 Lead 1969 Plexi",
                "gear_model": "Marshall JMP-50 Lead 1969 Plexi"}"#,
            ),
            "fixture",
        );
        assert_eq!(
            info.gear.as_deref(),
            Some("Marshall JMP-50 Lead 1969 Plexi")
        );
    }

    #[test]
    fn one_sided_gear_still_shows() {
        let info = ModelInfo::from_model(
            &model_with_metadata(r#"{"gear_make": "Fender", "gear_model": "tz-model"}"#),
            "fixture",
        );
        assert_eq!(info.gear.as_deref(), Some("Fender"));
    }

    #[test]
    fn blank_strings_count_as_absent() {
        let info = ModelInfo::from_model(
            &model_with_metadata(r#"{"gear_make": "  ", "gear_model": "", "modeled_by": " "}"#),
            "fixture",
        );
        assert_eq!(info.gear, None);
        assert_eq!(info.modeled_by, None);
        assert!(info.is_empty());
    }

    #[test]
    fn absent_metadata_yields_an_empty_summary() {
        let info = ModelInfo::from_model(&model_with_metadata("null"), "fixture");
        assert_eq!(info, ModelInfo::default());
        assert!(info.is_empty());
        assert_eq!(info.includes_cab, None);
    }

    /// Real case: TONE3000 uploads where the user filled in nothing leave
    /// `gear_type` at its default `amp` while the name records the truth. Trusting
    /// `amp` there would tell the user to add an IR to a capture that already has a
    /// cab — the exact double-cab this feature exists to prevent.
    #[test]
    fn name_overrides_gear_type_when_no_metadata_was_filled_in() {
        let info = ModelInfo::from_model(
            &model_with_metadata(
                r#"{"name": "Boosted-6505+-A2-Chugs-FullRig", "gear_type": "amp",
                    "gear_make": "T3K-Null", "gear_model": "T3K-Null",
                    "tone_type": "T3K-Null", "modeled_by": "ampspedalspickups"}"#,
            ),
            "Boosted-6505+-A2-Chugs-FullRig",
        );
        assert_eq!(info.includes_cab, Some(true));
        assert!(info.cab_from_name, "must be marked as inferred, not fact");
    }

    /// The other side of that rule: when the uploader *did* fill the form in,
    /// `gear_type` is a real statement and the name must not override it.
    #[test]
    fn real_metadata_beats_a_cab_sounding_name() {
        let info = ModelInfo::from_model(
            &model_with_metadata(
                r#"{"gear_type": "amp", "gear_make": "Mesa Boogie",
                    "gear_model": "Badlander", "tone_type": "hi_gain"}"#,
            ),
            "Mesa Badlander FullRig Cab",
        );
        assert_eq!(info.includes_cab, Some(false));
        assert!(!info.cab_from_name);
    }

    /// With no `gear_type` at all there is nothing to override, so the name is the
    /// only evidence available.
    #[test]
    fn name_is_consulted_when_gear_type_is_absent() {
        let info = ModelInfo::from_model(&model_with_metadata("{}"), "Plexi 4x12 Cab");
        assert_eq!(info.includes_cab, Some(true));
        assert!(info.cab_from_name);
    }

    /// The matcher stays narrow on purpose: a false "has a cab" costs the user
    /// their IR, so it must not read cabs into names that merely contain the letters.
    #[test]
    fn name_matching_is_narrow() {
        for name in ["Cabernet Overdrive", "Scab Fuzz", "Caballero Clean"] {
            let info = ModelInfo::from_model(&model_with_metadata("{}"), name);
            assert_eq!(info.includes_cab, None, "{name} must not read as a cab");
        }
        for name in [
            "Marshall FAT CAB",
            "6505 full_rig",
            "JCM800 fullrig",
            "Twin 2x12 cabinet",
        ] {
            let info = ModelInfo::from_model(&model_with_metadata("{}"), name);
            assert_eq!(info.includes_cab, Some(true), "{name} should read as a cab");
        }
    }

    /// A metadata `name` is better evidence than the filename, which may have been
    /// renamed on disk — but the filename still serves when there's no name.
    #[test]
    fn metadata_name_is_preferred_over_the_file_stem() {
        let info = ModelInfo::from_model(
            &model_with_metadata(r#"{"name": "Plexi FullRig"}"#),
            "renamed-on-disk",
        );
        assert_eq!(info.includes_cab, Some(true));
    }

    #[test]
    fn cab_inclusive_gear_is_flagged() {
        for gear in ["amp_cab", "full-rig", "cab"] {
            let info = ModelInfo::from_model(
                &model_with_metadata(&format!(r#"{{"gear_type": "{gear}"}}"#)),
                "fixture",
            );
            assert_eq!(info.includes_cab, Some(true), "{gear}");
        }
    }
}
