use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::gui::config::StageConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub stages: Vec<StageConfig>,
}

impl Preset {
    pub fn new(name: String, stages: Vec<StageConfig>) -> Self {
        Self {
            name,
            description: None,
            author: None,
            stages,
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }
}

pub struct PresetManager {
    presets_dir: PathBuf,
    presets: Vec<Preset>,
}

impl PresetManager {
    pub fn new<P: AsRef<Path>>(presets_dir: P) -> Result<Self> {
        let presets_dir = presets_dir.as_ref().to_path_buf();
        fs::create_dir_all(&presets_dir).context("Failed to create presets directory")?;

        let mut manager = Self {
            presets_dir,
            presets: Vec::new(),
        };

        manager.load_presets()?;
        manager.ensure_default_presets()?;

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
                        warn!("Failed to load preset {path:?}: {e}");
                    }
                }
            }
        }

        // Sort presets by name
        self.presets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    fn load_preset_file<P: AsRef<Path>>(&self, path: P) -> Result<Preset> {
        let content = fs::read_to_string(path.as_ref()).context("Failed to read preset file")?;

        serde_json::from_str(&content).context("Failed to parse preset JSON")
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
            Err(anyhow::anyhow!("Preset file not found: {}", preset_name))
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

    fn ensure_default_presets(&mut self) -> Result<()> {
        // Check if we already have presets
        if !self.presets.is_empty() {
            return Ok(());
        }

        // Create default presets
        let default_presets = create_default_presets();

        for preset in default_presets {
            self.save_preset(&preset)?;
        }

        Ok(())
    }
}

fn create_default_presets() -> Vec<Preset> {
    use crate::gui::config::*;
    use crate::sim::stages::clipper::ClipperType;
    use crate::sim::stages::filter::FilterType;
    use crate::sim::stages::tonestack::ToneStackModel;

    vec![
        // Clean preset
        Preset::new(
            "Clean".to_string(),
            vec![
                StageConfig::Filter(FilterConfig {
                    filter_type: FilterType::Highpass,
                    cutoff_hz: 80.0,
                    resonance: 0.0,
                }),
                StageConfig::Preamp(PreampConfig {
                    gain: 2.0,
                    bias: 0.0,
                    clipper_type: ClipperType::Soft,
                }),
                StageConfig::ToneStack(ToneStackConfig {
                    model: ToneStackModel::American,
                    bass: 0.6,
                    mid: 0.5,
                    treble: 0.7,
                    presence: 0.4,
                }),
                StageConfig::Level(LevelConfig { gain: 0.8 }),
            ],
        )
        .with_description("Clean and pristine tone with subtle warmth")
        .with_author("Rustortion"),
        // Crunch preset
        Preset::new(
            "Crunch".to_string(),
            vec![
                StageConfig::Filter(FilterConfig {
                    filter_type: FilterType::Highpass,
                    cutoff_hz: 100.0,
                    resonance: 0.1,
                }),
                StageConfig::Preamp(PreampConfig {
                    gain: 6.0,
                    bias: 0.1,
                    clipper_type: ClipperType::Medium,
                }),
                StageConfig::ToneStack(ToneStackConfig {
                    model: ToneStackModel::British,
                    bass: 0.5,
                    mid: 0.7,
                    treble: 0.6,
                    presence: 0.5,
                }),
                StageConfig::Compressor(CompressorConfig {
                    attack_ms: 2.0,
                    release_ms: 50.0,
                    threshold_db: -15.0,
                    ratio: 3.0,
                    makeup_db: 3.0,
                }),
                StageConfig::Level(LevelConfig { gain: 0.9 }),
            ],
        )
        .with_description("Classic rock crunch with mid-forward character")
        .with_author("Rustortion"),
        // Lead preset
        Preset::new(
            "Lead".to_string(),
            vec![
                StageConfig::Filter(FilterConfig {
                    filter_type: FilterType::Highpass,
                    cutoff_hz: 120.0,
                    resonance: 0.2,
                }),
                StageConfig::Preamp(PreampConfig {
                    gain: 8.5,
                    bias: 0.0,
                    clipper_type: ClipperType::Asymmetric,
                }),
                StageConfig::ToneStack(ToneStackConfig {
                    model: ToneStackModel::Modern,
                    bass: 0.4,
                    mid: 0.8,
                    treble: 0.7,
                    presence: 0.6,
                }),
                StageConfig::Compressor(CompressorConfig {
                    attack_ms: 1.0,
                    release_ms: 80.0,
                    threshold_db: -12.0,
                    ratio: 4.0,
                    makeup_db: 6.0,
                }),
                StageConfig::Level(LevelConfig { gain: 1.1 }),
            ],
        )
        .with_description("High-gain lead tone with sustain and clarity")
        .with_author("Rustortion"),
    ]
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            ' ' => '_',
            _ => '_',
        })
        .collect()
}
