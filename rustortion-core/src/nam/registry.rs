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
