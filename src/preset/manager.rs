use super::Preset;
use crate::gui::components::input_filter_control::InputFilterConfig;
use crate::gui::stages::StageCategory;
use anyhow::{Context, Result};
use log::warn;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Manager {
    presets_dir: PathBuf,
    presets: Vec<Preset>,
}

impl Manager {
    pub fn new(preset_dir: &str) -> Result<Self> {
        let presets_dir = Path::new(preset_dir).to_path_buf();
        fs::create_dir_all(&presets_dir).context("Failed to create presets directory")?;

        let mut manager = Self {
            presets_dir,
            presets: Vec::new(),
        };

        manager.load_presets()?;

        Ok(manager)
    }

    pub fn load_presets(&mut self) -> Result<()> {
        self.presets.clear();

        if !self.presets_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.presets_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_preset_file(&path) {
                    Ok(preset) => self.presets.push(preset),
                    Err(e) => {
                        warn!("Failed to load preset {}: {e}", path.display());
                    }
                }
            }
        }

        // Sort presets by name
        self.presets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn load_preset_file<P: AsRef<Path>>(&self, path: P) -> Result<Preset> {
        let content = fs::read_to_string(path.as_ref()).context("Failed to read preset file")?;

        let mut preset: Preset = if let Ok(preset) = serde_json::from_str(&content) {
            preset
        } else {
            // Try migration: parse as Value, strip Filter entries, extract input filters
            let mut value: serde_json::Value =
                serde_json::from_str(&content).context("Failed to parse preset JSON")?;
            migrate_preset(&mut value);
            serde_json::from_value(value).context("Failed to parse migrated preset")?
        };

        enforce_stage_ordering(&mut preset);
        Ok(preset)
    }

    pub fn save_preset(&mut self, preset: &Preset) -> Result<()> {
        let filename = format!("{}.json", sanitize_filename(&preset.name));
        let path = self.presets_dir.join(filename);

        let json = serde_json::to_string_pretty(preset).context("Failed to serialize preset")?;

        fs::write(&path, json).context("Failed to write preset file")?;

        // Reload presets to include the new/updated one
        self.load_presets()?;

        Ok(())
    }

    pub fn delete_preset(&mut self, preset_name: &str) -> Result<()> {
        let filename = format!("{}.json", sanitize_filename(preset_name));
        let path = self.presets_dir.join(filename);

        if path.exists() {
            fs::remove_file(&path).context("Failed to delete preset file")?;

            // Reload presets to reflect the deletion
            self.load_presets()?;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Preset file not found: {preset_name}"))
        }
    }

    pub fn preset_exists(&self, name: &str) -> bool {
        self.presets.iter().any(|p| p.name == name)
    }

    pub fn get_presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn get_preset_by_name(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

/// Migrate old preset format: strip `"Filter"` entries from stages and extract
/// highpass/lowpass cutoffs into an `input_filters` field.
fn migrate_preset(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    // Already migrated?
    if obj.contains_key("input_filters") {
        return;
    }

    let Some(stages) = obj.get("stages").and_then(|s| s.as_array()).cloned() else {
        return;
    };

    let mut hp_cutoff: Option<f32> = None;
    let mut lp_cutoff: Option<f32> = None;
    let mut non_filter_stages = Vec::new();

    for stage in &stages {
        if let Some(filter_obj) = stage.get("Filter") {
            let filter_type = filter_obj
                .get("filter_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cutoff = filter_obj
                .get("cutoff_hz")
                .and_then(serde_json::Value::as_f64)
                .map(|v| v as f32);

            match filter_type {
                "Highpass" if hp_cutoff.is_none() => {
                    hp_cutoff = cutoff;
                }
                "Lowpass" if lp_cutoff.is_none() => {
                    lp_cutoff = cutoff;
                }
                _ => {
                    // Drop additional/duplicate filter stages
                }
            }
        } else {
            non_filter_stages.push(stage.clone());
        }
    }

    // Build input_filters
    let input_filters = InputFilterConfig {
        hp_enabled: hp_cutoff.is_some(),
        hp_cutoff: hp_cutoff.unwrap_or(100.0),
        lp_enabled: lp_cutoff.is_some(),
        lp_cutoff: lp_cutoff.unwrap_or(8000.0),
    };

    obj.insert(
        "stages".to_string(),
        serde_json::to_value(non_filter_stages).unwrap(),
    );
    obj.insert(
        "input_filters".to_string(),
        serde_json::to_value(input_filters).unwrap(),
    );
}

/// Enforce stage ordering: Amp stages first, then Effect stages.
/// Preserves relative order within each category.
fn enforce_stage_ordering(preset: &mut Preset) {
    let mut amp_stages = Vec::new();
    let mut effect_stages = Vec::new();

    for stage in preset.stages.drain(..) {
        match stage.category() {
            StageCategory::Amp => amp_stages.push(stage),
            StageCategory::Effect => effect_stages.push(stage),
        }
    }

    preset.stages = amp_stages;
    preset.stages.append(&mut effect_stages);
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_preset_extracts_filters() {
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{
                "name": "Test",
                "stages": [
                    {"Filter": {"filter_type": "Highpass", "cutoff_hz": 150.0}},
                    {"Filter": {"filter_type": "Lowpass", "cutoff_hz": 7000.0}},
                    {"Preamp": {"gain": 1.0, "bias": 0.0, "clipper_type": "ClassA"}},
                    {"Level": {"gain": 1.0}}
                ],
                "ir_name": null,
                "ir_gain": 0.1,
                "pitch_shift_semitones": 0
            }"#,
        )
        .unwrap();

        migrate_preset(&mut value);

        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("input_filters"));

        let filters: InputFilterConfig =
            serde_json::from_value(obj["input_filters"].clone()).unwrap();
        assert!(filters.hp_enabled);
        assert!((filters.hp_cutoff - 150.0).abs() < f32::EPSILON);
        assert!(filters.lp_enabled);
        assert!((filters.lp_cutoff - 7000.0).abs() < f32::EPSILON);

        let stages = obj["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 2);
        assert!(stages[0].get("Preamp").is_some());
        assert!(stages[1].get("Level").is_some());
    }

    #[test]
    fn test_migrate_preset_skips_if_already_migrated() {
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{
                "name": "Test",
                "stages": [{"Preamp": {"gain": 1.0, "bias": 0.0, "clipper_type": "ClassA"}}],
                "input_filters": {"hp_enabled": true, "hp_cutoff": 100.0, "lp_enabled": false, "lp_cutoff": 8000.0},
                "ir_name": null,
                "ir_gain": 0.1,
                "pitch_shift_semitones": 0
            }"#,
        )
        .unwrap();

        migrate_preset(&mut value);

        let stages = value["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 1);
    }
}
