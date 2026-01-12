// src/gui/settings.rs
use anyhow::{Context, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::midi::MidiMapping;

impl std::fmt::Display for AudioSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Input Port: {}", self.input_port)?;
        writeln!(f, "Output Left Port: {}", self.output_left_port)?;
        writeln!(f, "Output Right Port: {}", self.output_right_port)?;
        writeln!(f, "Metronome Output Port: {}", self.metronome_out_port)?;
        writeln!(f, "Buffer Size: {}", self.buffer_size)?;
        writeln!(f, "Sample Rate: {}", self.sample_rate)?;
        writeln!(f, "Oversampling Factor: {}", self.oversampling_factor)?;
        Ok(())
    }
}

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

impl std::fmt::Display for MidiSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Controller Name: {}",
            self.controller_name.as_deref().unwrap_or("None")
        )?;
        writeln!(f, "Mappings:")?;
        for mapping in &self.mappings {
            writeln!(f, "  {:?}", mapping)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MidiSettings {
    /// The name of the selected MIDI controller
    pub controller_name: Option<String>,
    /// MIDI input to preset mappings
    pub mappings: Vec<MidiMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub audio: AudioSettings,
    pub midi: MidiSettings,
    pub recording_dir: String,
    pub ir_dir: String,
    pub preset_dir: String,
    pub ir_bypassed: bool,
    pub selected_preset: Option<String>,
}

impl std::fmt::Display for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "------------------------------")?;

        writeln!(f, "Audio Settings:")?;
        writeln!(f, "{}", self.audio)?;

        writeln!(f, "MIDI Settings:")?;
        writeln!(f, "{}", self.midi)?;

        writeln!(f, "Settings:")?;
        writeln!(f, "Recording Directory: {}", self.recording_dir)?;
        writeln!(f, "Impulse Response Directory: {}", self.ir_dir)?;
        writeln!(f, "Preset Directory: {}", self.preset_dir)?;
        writeln!(f, "IR Bypassed: {}", self.ir_bypassed)?;
        writeln!(
            f,
            "Selected Preset: {}",
            self.selected_preset.as_deref().unwrap_or("None")
        )?;
        Ok(())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            midi: MidiSettings::default(),
            recording_dir: "./recordings".to_string(),
            ir_dir: "./impulse_responses".to_string(),
            preset_dir: "./presets".to_string(),
            ir_bypassed: false,
            selected_preset: None,
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let settings_path = Self::get_settings_path();

        if settings_path.exists() {
            let contents =
                fs::read_to_string(&settings_path).context("Failed to read settings file")?;
            let settings: Settings =
                serde_json::from_str(&contents).context("Failed to parse settings")?;
            debug!("Loaded settings from {:?}", settings_path);
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
        let settings_path = Self::get_settings_path();

        // Ensure the config directory exists
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let json = serde_json::to_string_pretty(self).context("Failed to serialize settings")?;

        fs::write(&settings_path, json).context("Failed to write settings file")?;

        debug!("Saved settings to {:?}", settings_path);
        Ok(())
    }

    fn get_settings_path() -> PathBuf {
        const SETTINGS_FILENAME: &str = "settings.json";

        // Try to use XDG config directory on Linux
        if let Ok(config_dir) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(config_dir)
                .join("rustortion")
                .join(SETTINGS_FILENAME)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("rustortion")
                .join(SETTINGS_FILENAME)
        } else {
            // Fallback to current directory
            PathBuf::from(".").join(SETTINGS_FILENAME)
        }
    }

    /// Half the deal of working with PipeWire JACK is setting the right environment variables
    pub fn apply_to_environment(&self) {
        unsafe {
            // Try and configure PipeWire JACK settings
            std::env::set_var("PIPEWIRE_LATENCY", self.get_pipewire_latency());
            if std::env::var("JACK_PROMISCUOUS_SERVER").is_err() {
                std::env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
            }
        }
    }

    fn get_pipewire_latency(&self) -> String {
        format!("{}/{}", self.audio.buffer_size, self.audio.sample_rate)
    }
}
