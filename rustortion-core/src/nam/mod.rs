//! NAM (Neural Amp Modeler) model loading and a process-global parsed-model
//! registry.
//!
//! `.nam` models are parsed (and the runtime model allocated) off the real-time thread.
//! The [`loader`] scans a directory and parses every `*.nam` file into memory at
//! startup; the [`registry`] makes those parsed models reachable from
//! `StageConfig::to_runtime`, which has no other handle to the loader.

pub mod loader;
pub mod registry;

pub use loader::NamLoader;
