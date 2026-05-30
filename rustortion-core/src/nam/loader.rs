use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{info, warn};
use nam_rs::NamModel;

/// Scans a directory for `*.nam` files and parses each into memory.
///
/// Parsing happens once, at construction (off the real-time thread). Models are
/// keyed by display name (the file stem). Unparseable files are skipped with a
/// warning rather than failing the whole scan, matching the IR loader's tolerant
/// behaviour.
pub struct NamLoader {
    models: BTreeMap<String, Arc<NamModel>>,
}

impl NamLoader {
    /// Scan `directory` and parse every `*.nam` file found.
    ///
    /// A missing directory yields an empty loader (warn, not error) so the app can
    /// run without a nam folder present.
    pub fn new(directory: &Path) -> Result<Self> {
        let mut models = BTreeMap::new();

        if !directory.is_dir() {
            warn!(
                "NAM directory '{}' does not exist; no models loaded",
                directory.display()
            );
            return Ok(Self { models });
        }

        let entries = std::fs::read_dir(directory)
            .with_context(|| format!("Failed to read NAM directory '{}'", directory.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("nam") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };

            match std::fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|json| NamModel::from_json_str(&json).map_err(anyhow::Error::from))
            {
                Ok(model) => {
                    info!(
                        "Loaded NAM model '{name}' ({} Hz)",
                        model.sample_rate() as u32
                    );
                    models.insert(name, Arc::new(model));
                }
                Err(e) => warn!("Skipping NAM file '{}': {e}", path.display()),
            }
        }

        Ok(Self { models })
    }

    /// Sorted list of available model display names.
    #[must_use]
    pub fn available_names(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }

    /// Look up a parsed model by display name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<NamModel>> {
        self.models.get(name).cloned()
    }

    /// All parsed models, for populating the global registry.
    pub fn models(&self) -> impl Iterator<Item = (&String, &Arc<NamModel>)> {
        self.models.iter()
    }
}
