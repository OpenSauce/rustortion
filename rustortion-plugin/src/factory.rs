use rust_embed::RustEmbed;
use rustortion_core::preset::Preset;

#[derive(RustEmbed)]
#[folder = "../presets/"]
#[include = "*.json"]
struct FactoryPresets;

#[derive(RustEmbed)]
#[folder = "../impulse_responses/"]
#[include = "*.wav"]
struct FactoryIrs;

/// Parse all embedded factory presets, sorted by name.
pub fn load_factory_presets() -> Vec<Preset> {
    let mut presets: Vec<Preset> = FactoryPresets::iter()
        .filter_map(|filename| {
            let file = FactoryPresets::get(&filename)?;
            let json = std::str::from_utf8(file.data.as_ref()).ok()?;
            let preset: Preset = serde_json::from_str(json)
                .inspect_err(|e| log::warn!("Failed to parse factory preset {filename}: {e}"))
                .ok()?;
            Some(preset)
        })
        .collect();
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    presets
}

/// List all embedded IR names (relative paths with `/` separators).
pub fn factory_ir_names() -> Vec<String> {
    let mut names: Vec<String> = FactoryIrs::iter().map(|f| f.to_string()).collect();
    names.sort();
    names
}

/// Get the raw bytes of an embedded IR file.
pub fn get_factory_ir(name: &str) -> Option<Vec<u8>> {
    FactoryIrs::get(name).map(|f| f.data.to_vec())
}
