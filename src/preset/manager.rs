use super::Preset;
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

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}
