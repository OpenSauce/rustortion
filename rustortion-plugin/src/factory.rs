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

#[cfg(test)]
mod tests {
    use super::{FactoryIrs, FactoryPresets, factory_ir_names, load_factory_presets};
    use rustortion_core::preset::Preset;

    /// `load_factory_presets` deliberately drops presets it cannot parse, so a
    /// serde rename in `StageConfig` would silently ship a smaller preset list
    /// instead of failing the build. This asserts on the raw embed so that
    /// schema drift is a loud test failure.
    #[test]
    fn every_embedded_preset_parses() {
        let mut failures = Vec::new();

        for filename in FactoryPresets::iter() {
            let Some(file) = FactoryPresets::get(&filename) else {
                failures.push(format!("{filename}: embedded but not retrievable"));
                continue;
            };

            let json = match std::str::from_utf8(file.data.as_ref()) {
                Ok(json) => json,
                Err(e) => {
                    failures.push(format!("{filename}: not valid UTF-8: {e}"));
                    continue;
                }
            };

            if let Err(e) = serde_json::from_str::<Preset>(json) {
                failures.push(format!("{filename}: {e}"));
            }
        }

        assert!(
            failures.is_empty(),
            "embedded factory presets failed to parse:\n  {}",
            failures.join("\n  ")
        );
    }

    /// Guards the `#[folder = "../presets/"]` path. If the embed matched
    /// nothing, every other assertion here would vacuously pass.
    #[test]
    fn factory_presets_are_not_empty() {
        assert!(
            FactoryPresets::iter().next().is_some(),
            "no presets embedded — check the RustEmbed folder path"
        );
        assert!(
            !load_factory_presets().is_empty(),
            "presets are embedded but none survived parsing"
        );
    }

    #[test]
    fn factory_presets_are_sorted_and_uniquely_named() {
        let presets = load_factory_presets();
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();

        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            names, sorted,
            "load_factory_presets must return sorted names"
        );

        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(
            sorted, deduped,
            "duplicate factory preset names — the GUI selects presets by name"
        );
    }

    /// Same vacuity guard for the IR embed, plus the round-trip every caller
    /// relies on: a name from `factory_ir_names` must resolve to bytes.
    #[test]
    fn every_factory_ir_name_resolves() {
        let names = factory_ir_names();
        assert!(
            !names.is_empty(),
            "no IRs embedded — check the RustEmbed folder path"
        );

        for name in &names {
            let data = FactoryIrs::get(name)
                .unwrap_or_else(|| panic!("IR {name} listed but not retrievable"));
            assert!(!data.data.is_empty(), "IR {name} is empty");
        }
    }
}
