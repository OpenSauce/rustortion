use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{info, warn};
use nam_rs::NamModel;

use super::info::ModelInfo;

/// Scans a directory tree for `*.nam` files and parses each into memory.
///
/// Parsing happens once, at construction (off the real-time thread). Models are
/// keyed by their path relative to the scanned folder, without the extension and
/// with `/` separators on every platform (`Pack/Clean`), so the UI can group them
/// by folder and two packs may each ship a `Clean.nam`. A top-level file is keyed
/// by its bare stem, which keeps presets saved before folders existed resolving.
/// Unparseable files are skipped with a warning rather than failing the whole
/// scan, matching the IR loader's tolerant behaviour.
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
        let models = BTreeMap::new();
        let info = BTreeMap::new();

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

        let mut loader = Self { models, info };
        loader.scan_dir(directory, directory)?;
        Ok(loader)
    }

    /// Walk `current` (a folder under `base`), parsing models and recursing into
    /// subfolders. Only the top-level read is a hard error: an unreadable subfolder
    /// is skipped with a warning so one bad pack cannot hide every other model.
    fn scan_dir(&mut self, current: &Path, base: &Path) -> Result<()> {
        let entries = std::fs::read_dir(current)
            .with_context(|| format!("Failed to read NAM directory '{}'", current.display()))?;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Skipping unreadable entry in NAM directory: {e}");
                    continue;
                }
            };
            let path = entry.path();
            // `file_type` does not follow symlinks, so a link to a directory is
            // neither a directory nor a file here and is skipped: a link pointing
            // back up the tree would otherwise be walked until the OS refuses,
            // parsing every model dozens of times.
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir {
                // Sync tools and desktop trash keep state in dot-folders
                // (`.stfolder`, `.Trash-1000`); nothing in them is the collection.
                let hidden = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with('.'));
                if hidden {
                    continue;
                }
                if let Err(e) = self.scan_dir(&path, base) {
                    warn!("Skipping NAM subfolder '{}': {e}", path.display());
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("nam") {
                continue;
            }
            // The cab-from-name guess reads the file's own name: a folder called
            // "Full Rig" says nothing about the files inside it.
            let (Some(stem), Some(name)) = (
                path.file_stem().and_then(|s| s.to_str()),
                Self::model_key(&path, base),
            ) else {
                warn!(
                    "Skipping NAM file '{}': path is not valid UTF-8",
                    path.display()
                );
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
                    self.info
                        .insert(name.clone(), Arc::new(ModelInfo::from_model(&model, stem)));
                    self.models.insert(name, Arc::new(model));
                }
                Err(e) => warn!("Skipping NAM file '{}': {e}", path.display()),
            }
        }

        Ok(())
    }

    /// The registry key for a model file: its path relative to `base`, extension
    /// dropped, with `/` separators regardless of platform.
    fn model_key(path: &Path, base: &Path) -> Option<String> {
        let relative = path.strip_prefix(base).ok()?.with_extension("");
        let mut key = String::new();
        for component in relative.components() {
            if !key.is_empty() {
                key.push('/');
            }
            key.push_str(component.as_os_str().to_str()?);
        }
        Some(key)
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
    /// Packs ship as a folder of files. A model inside one is keyed by its path
    /// relative to the nam folder (forward slashes, no extension), so the UI can
    /// group by folder and two packs can both ship a `Clean.nam`. Top-level files
    /// keep their bare stem: a saved preset that names one keeps resolving.
    #[test]
    fn nested_directories_are_scanned_with_folder_prefixed_names() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference_standard.nam");

        std::fs::copy(&fixture, dir.join("top.nam")).expect("copy");
        std::fs::create_dir_all(dir.join("Pack/Deep")).expect("mkdir");
        std::fs::copy(&fixture, dir.join("Pack/inner.nam")).expect("copy");
        std::fs::copy(&fixture, dir.join("Pack/Deep/leaf.nam")).expect("copy");
        // Folders without any models contribute nothing.
        std::fs::create_dir_all(dir.join("Empty")).expect("mkdir");

        let loader = NamLoader::new(dir).expect("scan");

        assert_eq!(
            loader.available_names(),
            vec!["Pack/Deep/leaf", "Pack/inner", "top"]
        );
        assert!(loader.get("Pack/Deep/leaf").is_some());
        assert!(loader.info("Pack/inner").is_some());
    }

    /// The name-based cab guess must look at the file's own name only. A folder
    /// called "Full Rig" says nothing about a `plain.nam` inside it, while a file
    /// called `…-FullRig.nam` is evidence wherever it lives.
    #[test]
    fn cab_guess_from_name_ignores_folder_names() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference_standard.nam");

        std::fs::create_dir_all(dir.join("FullRig")).expect("mkdir");
        std::fs::copy(&fixture, dir.join("FullRig/plain.nam")).expect("copy");
        std::fs::create_dir_all(dir.join("Pack")).expect("mkdir");
        std::fs::copy(&fixture, dir.join("Pack/Amp-FullRig.nam")).expect("copy");

        let loader = NamLoader::new(dir).expect("scan");

        let plain = loader.info("FullRig/plain").expect("loaded");
        assert_eq!(plain.includes_cab, None, "folder name is not evidence");
        assert!(!plain.cab_from_name);

        let rig = loader.info("Pack/Amp-FullRig").expect("loaded");
        assert_eq!(rig.includes_cab, Some(true), "file name is evidence");
        assert!(rig.cab_from_name);
    }
    /// Sync tools and desktop trash leave dot-folders (`.stfolder`, `.Trash-1000`)
    /// inside the nam folder. Models in them are not the user's collection: a
    /// trashed file must not come back as a picker entry.
    #[test]
    fn hidden_directories_are_skipped() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference_standard.nam");

        std::fs::create_dir_all(dir.join(".Trash-1000")).expect("mkdir");
        std::fs::copy(&fixture, dir.join(".Trash-1000/deleted.nam")).expect("copy");
        std::fs::copy(&fixture, dir.join("kept.nam")).expect("copy");

        let loader = NamLoader::new(dir).expect("scan");

        assert_eq!(loader.available_names(), vec!["kept"]);
    }

    /// A symlink pointing back up the tree would otherwise be followed until the
    /// kernel refuses, parsing every model dozens of times into memory. Symlinked
    /// directories are skipped outright: the models they point at are reachable
    /// through their real path when they live inside the nam folder.
    #[cfg(unix)]
    #[test]
    fn symlinked_directories_are_not_followed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference_standard.nam");

        std::fs::create_dir_all(dir.join("Pack")).expect("mkdir");
        std::fs::copy(&fixture, dir.join("Pack/inner.nam")).expect("copy");
        std::os::unix::fs::symlink(dir, dir.join("Pack/loop")).expect("symlink");

        let loader = NamLoader::new(dir).expect("scan");

        assert_eq!(loader.available_names(), vec!["Pack/inner"]);
    }
}
