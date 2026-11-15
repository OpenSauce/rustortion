//! Process-global registry of parsed NAM models.
//!
//! `StageConfig::to_runtime(sample_rate)` builds stages off the real-time thread but
//! has no handle to the [`NamLoader`](super::loader::NamLoader). Since the nam folder
//! is a singleton resource, a process-global registry lets `NamConfig::to_runtime`
//! resolve a model by name without threading a loader through every `to_runtime` call
//! site.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use nam_rs::NamModel;

use super::info::ModelInfo;
use super::loader::NamLoader;

type Store = RwLock<HashMap<String, Arc<NamModel>>>;
type InfoStore = RwLock<HashMap<String, Arc<ModelInfo>>>;

static NAM_REGISTRY: OnceLock<Store> = OnceLock::new();
static NAM_INFO: OnceLock<InfoStore> = OnceLock::new();

fn store() -> &'static Store {
    NAM_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn info_store() -> &'static InfoStore {
    NAM_INFO.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Populate (or replace) the global registry from a loader's parsed models.
pub fn init_from_loader(loader: &NamLoader) {
    let mut map = store()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.clear();
    for (name, model) in loader.models() {
        map.insert(name.clone(), Arc::clone(model));
    }
    drop(map);

    let mut infos = info_store()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    infos.clear();
    for (name, info) in loader.infos() {
        infos.insert(name.clone(), Arc::clone(info));
    }
}

/// Cached display summary for a model, by display name.
///
/// Cheap enough to call from a GUI `view()`: the metadata was parsed once, at load.
#[must_use]
pub fn info(name: &str) -> Option<Arc<ModelInfo>> {
    let map = info_store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.get(name).cloned()
}

/// Look up a parsed model by display name.
#[must_use]
pub fn get(name: &str) -> Option<Arc<NamModel>> {
    let map = store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.get(name).cloned()
}

/// Sorted list of available model display names.
#[must_use]
pub fn available_names() -> Vec<String> {
    let map = store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut names: Vec<String> = map.keys().cloned().collect();
    names.sort();
    names
}

/// Every registered model's key and native sample rate in Hz, sorted by key.
///
/// One lock acquisition, so a GUI list needn't call [`get`] per row.
#[must_use]
pub fn catalog() -> Vec<(String, u32)> {
    let map = store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut entries: Vec<(String, u32)> = map
        .iter()
        .map(|(name, model)| (name.clone(), model.expected_sample_rate() as u32))
        .collect();
    drop(map);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses the same fixture dir as the NAM stage parity test, so the two tests
    /// leave the process-global registry in the same state whichever runs first.
    #[test]
    fn catalog_lists_every_model_with_its_native_rate_sorted_by_key() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let loader = NamLoader::new(&dir).expect("fixture dir loads");
        init_from_loader(&loader);

        let entries = catalog();

        let names: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, available_names());
        assert!(!entries.is_empty(), "fixture model must be listed");
        for (key, rate) in &entries {
            let model = get(key).expect("listed model is registered");
            assert_eq!(*rate, model.expected_sample_rate() as u32);
        }
    }
}
