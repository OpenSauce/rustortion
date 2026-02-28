use iced::keyboard::{Key, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeyMapping {
    /// Serialized key name (e.g. "F1", "1", "a")
    pub key: String,
    /// Modifier keys (e.g. ["Ctrl"], ["Shift", "Alt"], or [])
    pub modifiers: Vec<String>,
    /// The preset name to load when this hotkey is triggered
    pub preset_name: String,
    /// Human-readable description (e.g. "Ctrl+F1")
    pub description: String,
}

impl HotkeyMapping {
    pub fn new(key: String, modifiers: Vec<String>, preset_name: String) -> Self {
        let description = format_description(&modifiers, &key);
        Self {
            key,
            modifiers,
            preset_name,
            description,
        }
    }

    /// Check if a key event matches this mapping
    pub fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        let key_str = serialize_key(key);
        let Some(key_str) = key_str else {
            return false;
        };
        if key_str != self.key {
            return false;
        }

        let expected_mods = deserialize_modifiers(&self.modifiers);
        // Compare relevant modifier flags
        modifiers.control() == expected_mods.control()
            && modifiers.alt() == expected_mods.alt()
            && modifiers.shift() == expected_mods.shift()
            && modifiers.logo() == expected_mods.logo()
    }
}

/// Serialize an iced key to a string for storage
pub fn serialize_key(key: &Key) -> Option<String> {
    match key {
        Key::Named(named) => Some(format!("{named:?}")),
        Key::Character(c) => Some(c.to_string()),
        Key::Unidentified => None,
    }
}

/// Serialize iced modifiers to a list of strings
pub fn serialize_modifiers(modifiers: Modifiers) -> Vec<String> {
    let mut mods = Vec::new();
    if modifiers.control() {
        mods.push("Ctrl".to_string());
    }
    if modifiers.alt() {
        mods.push("Alt".to_string());
    }
    if modifiers.shift() {
        mods.push("Shift".to_string());
    }
    if modifiers.logo() {
        mods.push("Super".to_string());
    }
    mods
}

/// Deserialize modifier strings back to iced Modifiers
fn deserialize_modifiers(mods: &[String]) -> Modifiers {
    let mut result = Modifiers::empty();
    for m in mods {
        match m.as_str() {
            "Ctrl" => result |= Modifiers::CTRL,
            "Alt" => result |= Modifiers::ALT,
            "Shift" => result |= Modifiers::SHIFT,
            "Super" => result |= Modifiers::LOGO,
            _ => {}
        }
    }
    result
}

/// Format a human-readable description like "Ctrl+Shift+F1"
fn format_description(modifiers: &[String], key: &str) -> String {
    if modifiers.is_empty() {
        key.to_string()
    } else {
        format!("{}+{}", modifiers.join("+"), key)
    }
}

/// Check if a key should be ignored during hotkey capture.
/// This includes modifier-only keys and keys reserved for dialog interaction.
pub fn is_uncapturable_key(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            iced::keyboard::key::Named::Shift
                | iced::keyboard::key::Named::Control
                | iced::keyboard::key::Named::Alt
                | iced::keyboard::key::Named::Super
                | iced::keyboard::key::Named::Meta
                | iced::keyboard::key::Named::Escape
                | iced::keyboard::key::Named::Tab
                | iced::keyboard::key::Named::Enter
                | iced::keyboard::key::Named::Space
        )
    )
}
