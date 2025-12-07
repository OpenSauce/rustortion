// src/gui/settings.rs
use anyhow::{Context, Result};
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub input_port: String,
    pub output_left_port: String,
    pub output_right_port: String,
    pub metronome_out_port: String,
    pub buffer_size: u32,
    pub sample_rate: u32,
    pub oversampling_factor: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            input_port: "system:capture_1".to_string(),
            output_left_port: "system:playback_1".to_string(),
            output_right_port: "system:playback_2".to_string(),
            metronome_out_port: "system:playback_1".to_string(),
            buffer_size: 128,
            sample_rate: 48000,
            oversampling_factor: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub audio: AudioSettings,
    pub recording_dir: String,
    pub ir_dir: String,
    pub preset_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            recording_dir: "./recordings".to_string(),
            ir_dir: "./impulse_responses".to_string(),
            preset_dir: "./presets".to_string(),
        }
    }
}

impl Settings {
    const SETTINGS_FILE: &'static str = "settings.json";

    pub fn load() -> Result<Self> {
        let settings_path = Self::settings_path()?;

        if settings_path.exists() {
            let contents =
                fs::read_to_string(&settings_path).context("Failed to read settings file")?;
            let settings: Settings =
                serde_json::from_str(&contents).context("Failed to parse settings")?;
            info!("Loaded settings from {:?}", settings_path);
            Ok(settings)
        } else {
            info!("No settings file found, using defaults");
            let settings = Settings::default();
            // Try to save defaults, but don't fail if we can't
            let _ = settings.save();
            Ok(settings)
        }
    }

    pub fn save(&self) -> Result<()> {
        let settings_path = Self::settings_path()?;

        // Ensure the config directory exists
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let json = serde_json::to_string_pretty(self).context("Failed to serialize settings")?;

        fs::write(&settings_path, json).context("Failed to write settings file")?;

        info!("Saved settings to {:?}", settings_path);
        Ok(())
    }

    fn settings_path() -> Result<PathBuf> {
        // Try to use XDG config directory on Linux
        if let Ok(config_dir) = std::env::var("XDG_CONFIG_HOME") {
            Ok(PathBuf::from(config_dir)
                .join("rustortion")
                .join(Self::SETTINGS_FILE))
        } else if let Ok(home) = std::env::var("HOME") {
            Ok(PathBuf::from(home)
                .join(".config")
                .join("rustortion")
                .join(Self::SETTINGS_FILE))
        } else {
            // Fallback to current directory
            Ok(PathBuf::from(".").join(Self::SETTINGS_FILE))
        }
    }

    pub fn get_pipewire_latency(&self) -> String {
        format!("{}/{}", self.audio.buffer_size, self.audio.sample_rate)
    }

    pub fn apply_to_environment(&self) {
        unsafe {
            std::env::set_var("PIPEWIRE_LATENCY", self.get_pipewire_latency());
            if std::env::var("JACK_PROMISCUOUS_SERVER").is_err() {
                std::env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
            }
            if std::env::var("RECORDING_DIR").is_err() {
                std::env::set_var("RECORDING_DIR", &self.recording_dir);
            }
        }
    }
}
