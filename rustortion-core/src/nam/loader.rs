use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{info, warn};
use nam_rs::NamModel;

use super::info::ModelInfo;

/// Scans a directory for `*.nam` files and parses each into memory.
///
/// Parsing happens once, at construction (off the real-time thread). Models are
/// keyed by display name (the file stem). Unparseable files are skipped with a
/// warning rather than failing the whole scan, matching the IR loader's tolerant
/// behaviour.
pub struct NamLoader {
    models: BTreeMap<String, Arc<NamModel>>,
    /// Display summaries, extracted once here rather than per GUI frame.
    info: BTreeMap<String, Arc<ModelInfo>>,
}

impl NamLoader {
    /// Scan `directory` and parse every `*.nam` file found.
    ///
    /// A missing directory is created rather than merely reported, so the folder the
    /// UI tells users to drop `.nam` files into actually exists for them to find —
    /// otherwise the app advertises a path that isn't there. This mirrors
    /// [`IrLoader::scan_ir_directory`](crate::ir::loader::IrLoader::scan_ir_directory).
    /// Either way the result is an empty loader, never an error: failing to create
    /// the folder (read-only home, permissions) must not stop the app from starting.
    pub fn new(directory: &Path) -> Result<Self> {
        let mut models = BTreeMap::new();
        let mut info = BTreeMap::new();

        if !directory.is_dir() {
            match std::fs::create_dir_all(directory) {
                Ok(()) => info!("NAM directory created at {}", directory.display()),
                Err(e) => warn!(
                    "NAM directory '{}' does not exist and could not be created: {e}",
                    directory.display()
                ),
            }
            return Ok(Self { models, info });
        }

        let entries = std::fs::read_dir(directory)
            .with_context(|| format!("Failed to read NAM directory '{}'", directory.display()))?;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Skipping unreadable entry in NAM directory: {e}");
                    continue;
                }
            };
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
                        model.expected_sample_rate() as u32
                    );
                    // Summarize now: `metadata_typed` re-parses the whole JSON, so
                    // the GUI must never do it per frame.
                    info.insert(name.clone(), Arc::new(ModelInfo::from_model(&model)));
                    models.insert(name, Arc::new(model));
                }
                Err(e) => warn!("Skipping NAM file '{}': {e}", path.display()),
            }
        }

        Ok(Self { models, info })
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

    /// Cached display summary for a model, by display name.
    #[must_use]
    pub fn info(&self, name: &str) -> Option<Arc<ModelInfo>> {
        self.info.get(name).cloned()
    }

    /// All display summaries, for populating the global registry.
    pub fn infos(&self) -> impl Iterator<Item = (&String, &Arc<ModelInfo>)> {
        self.info.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The UI tells users to drop `.nam` files into this folder, so it has to exist
    /// for them to find. Advertising a path that isn't there is the confusing part.
    #[test]
    fn missing_directory_is_created() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("nam");
        assert!(!dir.exists(), "precondition: directory must not exist yet");

        let loader = NamLoader::new(&dir).expect("must not error on a missing dir");

        assert!(dir.is_dir(), "the directory should have been created");
        assert!(loader.available_names().is_empty());
    }

    /// Creation is a convenience, never a hard requirement: if the folder can't be
    /// made, the app still has to start.
    #[test]
    fn uncreatable_directory_is_not_an_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // A file, not a directory — so `create_dir_all` underneath it must fail.
        let file = tmp.path().join("not-a-dir");
        std::fs::write(&file, b"x").expect("write");

        let loader = NamLoader::new(&file.join("nam")).expect("must degrade, not error");
        assert!(loader.available_names().is_empty());
    }

    #[test]
    fn existing_directory_is_scanned_and_summarized() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        // The vendored reference model has no metadata block at all, which is the
        // case that must degrade to "unknown" rather than to a wrong answer.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let src = fixture.join("reference_standard.nam");
        std::fs::copy(&src, dir.join("reference_standard.nam")).expect("copy fixture");
        // A non-`.nam` file must be ignored rather than warned about as a model.
        std::fs::write(dir.join("notes.txt"), b"ignore me").expect("write");

        let loader = NamLoader::new(dir).expect("scan");

        assert_eq!(loader.available_names(), vec!["reference_standard"]);
        let info = loader
            .info("reference_standard")
            .expect("summary is cached");
        assert_eq!(
            info.includes_cab, None,
            "no metadata means unknown, not false"
        );
        assert!(info.is_empty());
        assert!(loader.info("nonexistent").is_none());
    }
}
