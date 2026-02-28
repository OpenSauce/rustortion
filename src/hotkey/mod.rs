use iced::keyboard::{Key, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyMapping {
    /// Serialized key name (e.g. "F1", "1", "a")
    pub key: String,
    /// Modifier keys (e.g. `["Ctrl"]`, `["Shift", "Alt"]`, or `[]`)
    pub modifiers: Vec<String>,
    /// The preset name to load when this hotkey is triggered
    pub preset_name: String,
    /// Human-readable description (e.g. "Ctrl+F1")
    pub description: String,
}

impl HotkeyMapping {
    pub fn new(key: String, mut modifiers: Vec<String>, preset_name: String) -> Self {
        // Canonicalize modifier order so comparisons and deduping are order-insensitive
        modifiers.sort();
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
pub const fn is_uncapturable_key(key: &Key) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::Named;

    #[test]
    fn test_serialize_key_named() {
        let key = Key::Named(Named::F1);
        assert_eq!(serialize_key(&key), Some("F1".to_string()));
    }

    #[test]
    fn test_serialize_key_character() {
        let key = Key::Character("a".into());
        assert_eq!(serialize_key(&key), Some("a".to_string()));
    }

    #[test]
    fn test_serialize_key_unidentified() {
        assert_eq!(serialize_key(&Key::Unidentified), None);
    }

    #[test]
    fn test_serialize_modifiers_none() {
        assert_eq!(
            serialize_modifiers(Modifiers::empty()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_serialize_modifiers_all() {
        let mods = Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::LOGO;
        let result = serialize_modifiers(mods);
        assert_eq!(result, vec!["Ctrl", "Alt", "Shift", "Super"]);
    }

    #[test]
    fn test_mapping_matches_simple_key() {
        let mapping = HotkeyMapping::new("F1".to_string(), vec![], "Clean".to_string());
        let key = Key::Named(Named::F1);
        assert!(mapping.matches(&key, Modifiers::empty()));
    }

    #[test]
    fn test_mapping_does_not_match_wrong_key() {
        let mapping = HotkeyMapping::new("F1".to_string(), vec![], "Clean".to_string());
        let key = Key::Named(Named::F2);
        assert!(!mapping.matches(&key, Modifiers::empty()));
    }

    #[test]
    fn test_mapping_matches_with_modifiers() {
        let mapping = HotkeyMapping::new(
            "F1".to_string(),
            vec!["Ctrl".to_string()],
            "Distorted".to_string(),
        );
        let key = Key::Named(Named::F1);
        assert!(mapping.matches(&key, Modifiers::CTRL));
    }

    #[test]
    fn test_mapping_does_not_match_missing_modifier() {
        let mapping = HotkeyMapping::new(
            "F1".to_string(),
            vec!["Ctrl".to_string()],
            "Distorted".to_string(),
        );
        let key = Key::Named(Named::F1);
        assert!(!mapping.matches(&key, Modifiers::empty()));
    }

    #[test]
    fn test_mapping_does_not_match_extra_modifier() {
        let mapping = HotkeyMapping::new("F1".to_string(), vec![], "Clean".to_string());
        let key = Key::Named(Named::F1);
        assert!(!mapping.matches(&key, Modifiers::CTRL));
    }

    #[test]
    fn test_modifiers_are_canonicalized() {
        let m1 = HotkeyMapping::new(
            "F1".to_string(),
            vec!["Shift".to_string(), "Ctrl".to_string()],
            "A".to_string(),
        );
        let m2 = HotkeyMapping::new(
            "F1".to_string(),
            vec!["Ctrl".to_string(), "Shift".to_string()],
            "B".to_string(),
        );
        // Both should have the same canonical modifier order
        assert_eq!(m1.modifiers, m2.modifiers);
    }

    #[test]
    fn test_description_no_modifiers() {
        let mapping = HotkeyMapping::new("F1".to_string(), vec![], "Clean".to_string());
        assert_eq!(mapping.description, "F1");
    }

    #[test]
    fn test_description_with_modifiers() {
        let mapping = HotkeyMapping::new(
            "F1".to_string(),
            vec!["Ctrl".to_string(), "Shift".to_string()],
            "Clean".to_string(),
        );
        assert_eq!(mapping.description, "Ctrl+Shift+F1");
    }

    #[test]
    fn test_is_uncapturable_key_modifiers() {
        assert!(is_uncapturable_key(&Key::Named(Named::Shift)));
        assert!(is_uncapturable_key(&Key::Named(Named::Control)));
        assert!(is_uncapturable_key(&Key::Named(Named::Alt)));
        assert!(is_uncapturable_key(&Key::Named(Named::Super)));
    }

    #[test]
    fn test_is_uncapturable_key_reserved() {
        assert!(is_uncapturable_key(&Key::Named(Named::Escape)));
        assert!(is_uncapturable_key(&Key::Named(Named::Tab)));
        assert!(is_uncapturable_key(&Key::Named(Named::Enter)));
        assert!(is_uncapturable_key(&Key::Named(Named::Space)));
    }

    #[test]
    fn test_is_capturable_key() {
        assert!(!is_uncapturable_key(&Key::Named(Named::F1)));
        assert!(!is_uncapturable_key(&Key::Character("a".into())));
    }

    #[test]
    fn test_mapping_character_key() {
        let mapping = HotkeyMapping::new("1".to_string(), vec![], "Clean".to_string());
        let key = Key::Character("1".into());
        assert!(mapping.matches(&key, Modifiers::empty()));
    }
}
