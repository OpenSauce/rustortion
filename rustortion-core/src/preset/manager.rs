use super::{InputFilterConfig, Preset, StageCategory};
use anyhow::{Context, Result};
use log::warn;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Manager {
    presets_dir: PathBuf,
    presets: Vec<Preset>,
    /// Maps a loaded preset's `name` to the file it was read from.
    ///
    /// Presets are identified by the `name` field *inside* the JSON, never by
    /// their filename, so the on-disk name is free to change between releases.
    /// Remembering where each preset actually came from lets saves and deletes
    /// target the existing file — including files written by older versions
    /// under a different sanitising scheme — instead of orphaning them.
    preset_paths: HashMap<String, PathBuf>,
}

impl Manager {
    pub fn new(preset_dir: impl AsRef<Path>) -> Result<Self> {
        let presets_dir = preset_dir.as_ref().to_path_buf();
        fs::create_dir_all(&presets_dir).context("Failed to create presets directory")?;

        let mut manager = Self {
            presets_dir,
            presets: Vec::new(),
            preset_paths: HashMap::new(),
        };

        manager.load_presets()?;

        Ok(manager)
    }

    /// Create a manager from an in-memory list of presets (no filesystem).
    /// Save/delete operations will return errors.
    #[allow(clippy::missing_const_for_fn)] // Vec::new() is not const-stable
    pub fn new_from_presets(presets: Vec<Preset>) -> Self {
        Self {
            presets_dir: PathBuf::new(),
            presets,
            preset_paths: HashMap::new(),
        }
    }

    pub fn load_presets(&mut self) -> Result<()> {
        self.presets.clear();
        self.preset_paths.clear();

        if !self.presets_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.presets_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_preset_file(&path) {
                    Ok(preset) => {
                        self.preset_paths.insert(preset.name.clone(), path);
                        self.presets.push(preset);
                    }
                    Err(e) => {
                        warn!("Failed to load preset {}: {e}", path.display());
                    }
                }
            }
        }

        // Sort presets by name
        self.presets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn load_preset_file<P: AsRef<Path>>(&self, path: P) -> Result<Preset> {
        let content = fs::read_to_string(path.as_ref()).context("Failed to read preset file")?;

        let mut preset: Preset = if let Ok(preset) = serde_json::from_str(&content) {
            preset
        } else {
            // Try migration: parse as Value, strip Filter entries, extract input filters
            let mut value: serde_json::Value =
                serde_json::from_str(&content).context("Failed to parse preset JSON")?;
            migrate_preset(&mut value);
            serde_json::from_value(value).context("Failed to parse migrated preset")?
        };

        enforce_stage_ordering(&mut preset);
        Ok(preset)
    }

    pub fn save_preset(&mut self, preset: &Preset) -> Result<()> {
        if self.presets_dir.as_os_str().is_empty() {
            return Err(anyhow::anyhow!("Cannot save presets in read-only mode"));
        }
        let path = self.path_for(&preset.name);

        let json = serde_json::to_string_pretty(preset).context("Failed to serialize preset")?;

        fs::write(&path, json).context("Failed to write preset file")?;

        // Reload presets to include the new/updated one
        self.load_presets()?;

        Ok(())
    }

    pub fn delete_preset(&mut self, preset_name: &str) -> Result<()> {
        if self.presets_dir.as_os_str().is_empty() {
            return Err(anyhow::anyhow!("Cannot delete presets in read-only mode"));
        }

        // Only ever remove a file we positively attributed to this preset when
        // it was loaded. Deriving a path from the name instead would let
        // `delete_preset("My_Preset")` delete `My_Preset.json` — which may well
        // hold a *different* preset named `My Preset` — and report success.
        let path = self
            .preset_paths
            .get(preset_name)
            .filter(|path| path.exists())
            .ok_or_else(|| anyhow::anyhow!("Preset file not found: {preset_name}"))?
            .clone();

        fs::remove_file(&path).context("Failed to delete preset file")?;

        // Reload presets to reflect the deletion
        self.load_presets()?;

        Ok(())
    }

    pub fn preset_exists(&self, name: &str) -> bool {
        self.presets.iter().any(|p| p.name == name)
    }

    pub fn get_presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn get_preset_by_name(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Resolve the file a preset should be written to.
    ///
    /// If the preset is already on disk we reuse the exact path it was loaded
    /// from, so presets saved by older versions keep their existing filenames
    /// and never end up duplicated.
    ///
    /// A preset we have never seen gets a freshly encoded filename — but only
    /// if nothing else already occupies it. Encoding is the identity on names
    /// drawn from `[A-Za-z0-9-_]`, which is exactly the alphabet the old lossy
    /// scheme emitted, so a *new* preset called `High_Gain` would otherwise
    /// land on the shipped `High_Gain.json` holding `High Gain` and destroy it.
    /// When the candidate is taken we fall back to the digest-suffixed stem
    /// instead. Filenames carry no user-visible identity — the `name` field
    /// inside the JSON does — so disambiguating silently is strictly
    /// data-preserving and invisible to the user. Same-*name* overwrites are a
    /// separate question, gated by the GUI's confirmation prompt.
    fn path_for(&self, preset_name: &str) -> PathBuf {
        if let Some(existing) = self.preset_paths.get(preset_name) {
            return existing.clone();
        }

        let candidate = self.file_for_stem(&sanitize_filename(preset_name));
        if self.is_unoccupied(&candidate) {
            return candidate;
        }

        let hashed = hashed_stem(preset_name);
        let candidate = self.file_for_stem(&hashed);
        if self.is_unoccupied(&candidate) {
            return candidate;
        }

        // The digest stem is taken too — only reachable if some preset is
        // literally named after it. Walk a counter; every step is still a
        // deterministic filename. Exhausting the range would need a thousand
        // such presets, so falling back to the digest stem is unreachable in
        // practice rather than a silent overwrite waiting to happen.
        (1..=MAX_DISAMBIGUATION_ATTEMPTS)
            .map(|n| self.file_for_stem(&format!("{hashed}-{n}")))
            .find(|path| self.is_unoccupied(path))
            .unwrap_or(candidate)
    }

    fn file_for_stem(&self, stem: &str) -> PathBuf {
        self.presets_dir.join(format!("{stem}.json"))
    }

    /// Is `path` free for a preset we are about to write?
    ///
    /// Callers reach this only after `path_for` has ruled out the preset's own
    /// file, so anything already on disk belongs to something else — including
    /// files that failed to parse and so never made it into `preset_paths`.
    fn is_unoccupied(&self, path: &Path) -> bool {
        !path.exists() && !self.preset_paths.values().any(|owned| owned == path)
    }
}

/// Migrate old preset format: strip `"Filter"` entries from stages and extract
/// highpass/lowpass cutoffs into an `input_filters` field.
fn migrate_preset(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    // Already migrated?
    if obj.contains_key("input_filters") {
        return;
    }

    let Some(stages) = obj.get("stages").and_then(|s| s.as_array()).cloned() else {
        return;
    };

    let mut hp_found = false;
    let mut lp_found = false;
    let mut hp_cutoff = 100.0_f32;
    let mut lp_cutoff = 8000.0_f32;
    let mut non_filter_stages = Vec::new();

    for stage in &stages {
        if let Some(filter_obj) = stage.get("Filter") {
            let filter_type = filter_obj
                .get("filter_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match filter_type {
                "Highpass" if !hp_found => {
                    hp_found = true;
                    if let Some(v) = filter_obj
                        .get("cutoff_hz")
                        .and_then(serde_json::Value::as_f64)
                    {
                        hp_cutoff = v as f32;
                    }
                }
                "Lowpass" if !lp_found => {
                    lp_found = true;
                    if let Some(v) = filter_obj
                        .get("cutoff_hz")
                        .and_then(serde_json::Value::as_f64)
                    {
                        lp_cutoff = v as f32;
                    }
                }
                _ => {
                    // Drop additional/duplicate filter stages
                }
            }
        } else {
            non_filter_stages.push(stage.clone());
        }
    }

    // Build input_filters — enabled tracks stage presence, cutoff defaults if missing
    let input_filters = InputFilterConfig {
        hp_enabled: hp_found,
        hp_cutoff,
        lp_enabled: lp_found,
        lp_cutoff,
    };

    obj.insert(
        "stages".to_string(),
        serde_json::Value::Array(non_filter_stages),
    );
    if let Ok(filters_value) = serde_json::to_value(input_filters) {
        obj.insert("input_filters".to_string(), filters_value);
    }
}

/// Enforce stage ordering: Amp stages first, then Effect stages.
/// Preserves relative order within each category.
fn enforce_stage_ordering(preset: &mut Preset) {
    let mut amp_stages = Vec::new();
    let mut effect_stages = Vec::new();

    for stage in preset.stages.drain(..) {
        match stage.category() {
            StageCategory::Amp => amp_stages.push(stage),
            StageCategory::Effect => effect_stages.push(stage),
        }
    }

    preset.stages = amp_stages;
    preset.stages.append(&mut effect_stages);
}

/// Bytes that survive into a filename verbatim. Deliberately the same set the
/// previous (lossy) scheme kept, so any preset name that was already safe keeps
/// the exact filename it has today.
const fn is_unreserved(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_')
}

/// Longest filename stem we will produce, in bytes. Almost every filesystem
/// caps a single path component at 255 bytes; this leaves room for `.json`
/// plus a little slack.
const MAX_STEM_BYTES: usize = 200;

/// `'-'` plus 16 hex digits, appended to truncated stems.
const HASH_SUFFIX_BYTES: usize = 17;

/// How far `Manager::path_for` will count when even the digest-suffixed stem is
/// already occupied. Reaching the end needs a preset named after the digest
/// stem *and* a thousand more named after its numbered variants.
const MAX_DISAMBIGUATION_ATTEMPTS: u32 = 1000;

const HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";

/// 64-bit FNV-1a.
///
/// Hand-rolled rather than using `DefaultHasher`, whose algorithm is explicitly
/// not stable across Rust releases — these hashes end up in filenames, so they
/// have to survive a toolchain upgrade.
fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Percent-encode `name` into at most `budget` bytes. Returns the encoded text
/// and whether it had to stop early. Only whole units (one plain byte or one
/// complete `%XX` triplet) are emitted, so the result is never a split escape.
fn percent_encode(name: &str, budget: usize) -> (String, bool) {
    let mut out = String::with_capacity(name.len());

    for &byte in name.as_bytes() {
        let width = if is_unreserved(byte) { 1 } else { 3 };
        if out.len() + width > budget {
            return (out, true);
        }

        if is_unreserved(byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX_DIGITS[(byte >> 4) as usize] as char);
            out.push(HEX_DIGITS[(byte & 0x0f) as usize] as char);
        }
    }

    (out, false)
}

/// A readable prefix of `name` plus a digest of the whole of it, short enough
/// to survive the filesystem's per-component byte limit.
///
/// Used both for pathologically long names and, by `Manager::path_for`, to step
/// aside when the plain encoding is already occupied.
fn hashed_stem(name: &str) -> String {
    let (stem, _) = percent_encode(name, MAX_STEM_BYTES - HASH_SUFFIX_BYTES);
    let hash = stable_hash(name.as_bytes());
    format!("{stem}-{hash:016x}")
}

/// Turn a preset name into a filename stem.
///
/// Percent-encoding every byte outside `[A-Za-z0-9-_]` is stable, reversible,
/// and — crucially for existing installs — the identity on names that only use
/// the allowed characters, which is exactly the set of names the old scheme
/// left untouched. The old scheme mapped everything else to `_`, which
/// collapsed *every* non-ASCII name (a whole locale's worth of preset names)
/// onto a single file.
///
/// **The mapping is not injective.** It is injective on names short enough to
/// encode whole, but longer names are truncated and digest-suffixed, and `-`
/// and `0-9a-f` are all unreserved — so the suffixed stem is itself a valid
/// identity encoding of some other, shorter name. `"x".repeat(500)` and a
/// preset named literally `xxx…x-b6822c71b0501b75` produce the same stem.
/// Callers must not assume distinct names yield distinct filenames;
/// `Manager::path_for` checks that its candidate is unoccupied before handing
/// it out, which is what actually keeps two presets off one file.
fn sanitize_filename(name: &str) -> String {
    if name.is_empty() {
        // The encoder never emits a bare `%` — every `%` it writes is followed
        // by two hex digits — so this cannot collide with a real name.
        return "%".to_owned();
    }

    let (encoded, truncated) = percent_encode(name, MAX_STEM_BYTES);
    if truncated {
        hashed_stem(name)
    } else {
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amp::stages::level::LevelConfig;
    use crate::preset::StageConfig;
    use std::collections::HashSet;

    fn preset_named(name: &str) -> Preset {
        Preset::new(
            name.to_owned(),
            vec![StageConfig::Level(LevelConfig::default())],
            Some("cab.wav".to_owned()),
            0.25,
            -2,
            InputFilterConfig::default(),
        )
    }

    fn json_files(dir: &Path) -> HashSet<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect()
    }

    /// Existing installs already have files on disk. Any name the old scheme
    /// left alone must still map to the same filename, or every ASCII preset
    /// would be orphaned on upgrade.
    #[test]
    fn sanitize_filename_leaves_already_safe_names_unchanged() {
        for name in ["Rock", "lead-tone_2", "ABC123", "_", "-", "a"] {
            assert_eq!(sanitize_filename(name), name, "name {name:?} was rewritten");
        }
    }

    /// The bug: every ZH_CN name used to collapse to a run of underscores, so a
    /// Chinese-locale user effectively had a single preset slot.
    #[test]
    fn sanitize_filename_distinguishes_zh_cn_names() {
        let metal = sanitize_filename("重金属");
        let clean = sanitize_filename("清音");

        assert_eq!(metal, "%E9%87%8D%E9%87%91%E5%B1%9E");
        assert_eq!(clean, "%E6%B8%85%E9%9F%B3");
        assert_ne!(metal, clean);
        // Nothing collapses to the old all-underscore stem.
        assert!(!metal.contains('_'));
        assert!(!clean.contains('_'));
    }

    #[test]
    fn sanitize_filename_distinguishes_punctuation_only_differences() {
        let names = [
            "My Preset",
            "My-Preset",
            "My_Preset",
            "My.Preset",
            "My/Preset",
            "My:Preset",
            "MyPreset",
            // `%` must itself be escaped or the encoding would not be injective:
            // "My%20Preset" and "My Preset" would otherwise share a file.
            "My%20Preset",
        ];

        let stems: HashSet<String> = names.iter().map(|n| sanitize_filename(n)).collect();
        assert_eq!(stems.len(), names.len(), "punctuation variants collided");

        assert_eq!(sanitize_filename("My Preset"), "My%20Preset");
        assert_eq!(sanitize_filename("My%20Preset"), "My%2520Preset");
        // A path separator can never survive into the filename.
        assert!(!sanitize_filename("My/Preset").contains('/'));
    }

    #[test]
    fn sanitize_filename_bounds_length_without_colliding() {
        let long_ascii = "x".repeat(500);
        let long_zh = "重".repeat(500);

        for stem in [
            sanitize_filename(&long_ascii),
            sanitize_filename(&long_zh),
            sanitize_filename(&format!("{long_ascii}y")),
        ] {
            assert!(
                stem.len() <= MAX_STEM_BYTES,
                "stem too long: {}",
                stem.len()
            );
        }

        // Names sharing a long prefix still get distinct filenames.
        assert_ne!(
            sanitize_filename(&format!("{long_ascii}a")),
            sanitize_filename(&format!("{long_ascii}b"))
        );
        assert_ne!(
            sanitize_filename(&format!("{long_zh}重")),
            sanitize_filename(&format!("{long_zh}金"))
        );
    }

    /// The empty name has no encoding of its own, so it gets a stem the encoder
    /// can never produce. Justified by a comment since the PR that added it,
    /// but never actually checked.
    #[test]
    fn sanitize_filename_handles_the_empty_name() {
        assert_eq!(sanitize_filename(""), "%");
        assert_eq!(sanitize_filename("%"), "%25");
        assert_ne!(sanitize_filename(""), sanitize_filename("%"));
    }

    /// `stable_hash` is hand-rolled FNV-1a precisely because `DefaultHasher`'s
    /// algorithm is not stable across Rust releases, and these digests end up in
    /// filenames. Determinism *within* a process proves nothing about that — pin
    /// literals so a toolchain that changed them fails here instead of silently
    /// orphaning every long-named preset.
    #[test]
    fn stable_hash_matches_pinned_values() {
        assert_eq!(stable_hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash(b"rustortion"), 0x79c8_6023_c73e_36c0);
        assert_eq!(stable_hash("重金属".as_bytes()), 0x980e_d7b0_0cf5_300d);
        assert_eq!(
            stable_hash("x".repeat(500).as_bytes()),
            0xb682_2c71_b050_1b75
        );

        // ...and the filename that digest produces.
        assert_eq!(
            sanitize_filename(&"x".repeat(500)),
            format!(
                "{}-b6822c71b0501b75",
                "x".repeat(MAX_STEM_BYTES - HASH_SUFFIX_BYTES)
            )
        );
    }

    #[test]
    fn sanitize_filename_is_deterministic() {
        assert_eq!(sanitize_filename("重金属"), sanitize_filename("重金属"));
        let long = "重".repeat(500);
        assert_eq!(sanitize_filename(&long), sanitize_filename(&long));
    }

    #[test]
    fn save_preset_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::new(dir.path()).unwrap();

        manager.save_preset(&preset_named("重金属 v2")).unwrap();

        // Re-read from scratch to prove the file, not the in-memory copy.
        let reloaded = Manager::new(dir.path()).unwrap();
        let preset = reloaded
            .get_preset_by_name("重金属 v2")
            .expect("preset missing after reload");

        assert_eq!(preset.name, "重金属 v2");
        assert_eq!(preset.stages.len(), 1);
        assert_eq!(preset.ir_name.as_deref(), Some("cab.wav"));
        assert!((preset.ir_gain - 0.25).abs() < f32::EPSILON);
        assert_eq!(preset.pitch_shift_semitones, -2);
    }

    #[test]
    fn distinct_zh_cn_presets_do_not_overwrite_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::new(dir.path()).unwrap();

        for name in ["重金属", "清音", "布鲁斯"] {
            manager.save_preset(&preset_named(name)).unwrap();
        }

        assert_eq!(manager.get_presets().len(), 3);
        let reloaded = Manager::new(dir.path()).unwrap();
        assert_eq!(reloaded.get_presets().len(), 3);
        for name in ["重金属", "清音", "布鲁斯"] {
            assert!(reloaded.get_preset_by_name(name).is_some(), "lost {name}");
        }
    }

    /// Backward compatibility: a preset written by an older version under the
    /// lossy scheme must be updated in place, not duplicated under a new name.
    #[test]
    fn saving_over_a_legacy_filename_reuses_that_file() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_path = dir.path().join("My_Preset.json");
        let legacy = Preset::new(
            "My Preset".to_owned(),
            Vec::new(),
            None,
            0.1,
            0,
            InputFilterConfig::default(),
        );
        fs::write(&legacy_path, serde_json::to_string(&legacy).unwrap()).unwrap();

        let mut manager = Manager::new(dir.path()).unwrap();
        assert_eq!(manager.get_presets().len(), 1);

        manager.save_preset(&preset_named("My Preset")).unwrap();

        assert!(legacy_path.exists(), "legacy file was orphaned");
        assert!(!dir.path().join("My%20Preset.json").exists());
        assert_eq!(manager.get_presets().len(), 1, "preset was duplicated");
        assert_eq!(
            manager
                .get_preset_by_name("My Preset")
                .unwrap()
                .stages
                .len(),
            1
        );
    }

    /// The path-occupancy hole. `presets/High_Gain.json` ships with the app and
    /// holds a preset *named* `High Gain`. Encoding is the identity on
    /// `High_Gain`, so saving a preset under that literal name computed the very
    /// same path — no dialog, since the overwrite gate compares names — and the
    /// factory preset was gone.
    #[test]
    fn saving_a_name_that_lands_on_another_presets_file_keeps_both() {
        let dir = tempfile::tempdir().unwrap();
        let factory_path = dir.path().join("High_Gain.json");
        fs::write(
            &factory_path,
            serde_json::to_string(&preset_named("High Gain")).unwrap(),
        )
        .unwrap();

        let mut manager = Manager::new(dir.path()).unwrap();
        manager.save_preset(&preset_named("High_Gain")).unwrap();

        // The shipped file still holds the shipped preset.
        let on_disk: Preset =
            serde_json::from_str(&fs::read_to_string(&factory_path).unwrap()).unwrap();
        assert_eq!(on_disk.name, "High Gain", "factory preset was clobbered");

        // The new preset stepped aside onto the digest-suffixed stem.
        let disambiguated = dir
            .path()
            .join(format!("High_Gain-{:016x}.json", stable_hash(b"High_Gain")));
        assert!(disambiguated.exists(), "no separate file for `High_Gain`");

        // Two presets, two files, nothing lost — proven through a fresh reload.
        assert_eq!(json_files(dir.path()).len(), 2);
        let reloaded = Manager::new(dir.path()).unwrap();
        assert_eq!(reloaded.get_presets().len(), 2);
        assert!(reloaded.get_preset_by_name("High Gain").is_some());
        assert!(reloaded.get_preset_by_name("High_Gain").is_some());
    }

    /// `sanitize_filename` is injective only below the truncation boundary: a
    /// digest-suffixed stem is itself a valid identity encoding. `path_for`'s
    /// occupancy check, not the encoding, is what keeps the two names apart.
    #[test]
    fn a_long_name_and_its_digest_stem_namesake_get_separate_files() {
        let long = "x".repeat(500);
        let namesake = sanitize_filename(&long);
        assert_eq!(
            sanitize_filename(&namesake),
            namesake,
            "expected the stems to collide"
        );

        let dir = tempfile::tempdir().unwrap();
        let mut manager = Manager::new(dir.path()).unwrap();
        manager.save_preset(&preset_named(&long)).unwrap();
        manager.save_preset(&preset_named(&namesake)).unwrap();

        assert_eq!(json_files(dir.path()).len(), 2);
        let reloaded = Manager::new(dir.path()).unwrap();
        assert_eq!(reloaded.get_presets().len(), 2);
        assert!(reloaded.get_preset_by_name(&long).is_some());
        assert!(reloaded.get_preset_by_name(&namesake).is_some());
    }

    /// Deleting by name must never fall back to a name-derived path: the file it
    /// computes may belong to a different preset entirely.
    #[test]
    fn deleting_a_name_that_lands_on_another_presets_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let other_path = dir.path().join("My_Preset.json");
        fs::write(
            &other_path,
            serde_json::to_string(&preset_named("My Preset")).unwrap(),
        )
        .unwrap();

        let mut manager = Manager::new(dir.path()).unwrap();
        let err = manager.delete_preset("My_Preset").unwrap_err();

        assert!(
            err.to_string().contains("Preset file not found"),
            "unexpected error: {err}"
        );
        assert!(other_path.exists(), "deleted another preset's file");
        assert!(manager.get_preset_by_name("My Preset").is_some());
    }

    #[test]
    fn deleting_a_legacy_filename_works() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_path = dir.path().join("____.json");
        let legacy = Preset::new(
            "重金属".to_owned(),
            Vec::new(),
            None,
            0.1,
            0,
            InputFilterConfig::default(),
        );
        fs::write(&legacy_path, serde_json::to_string(&legacy).unwrap()).unwrap();

        let mut manager = Manager::new(dir.path()).unwrap();
        manager.delete_preset("重金属").unwrap();

        assert!(!legacy_path.exists());
        assert!(manager.get_presets().is_empty());
    }

    #[test]
    fn test_migrate_preset_extracts_filters() {
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{
                "name": "Test",
                "stages": [
                    {"Filter": {"filter_type": "Highpass", "cutoff_hz": 150.0}},
                    {"Filter": {"filter_type": "Lowpass", "cutoff_hz": 7000.0}},
                    {"Preamp": {"gain": 1.0, "bias": 0.0, "clipper_type": "ClassA"}},
                    {"Level": {"gain": 1.0}}
                ],
                "ir_name": null,
                "ir_gain": 0.1,
                "pitch_shift_semitones": 0
            }"#,
        )
        .unwrap();

        migrate_preset(&mut value);

        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("input_filters"));

        let filters: InputFilterConfig =
            serde_json::from_value(obj["input_filters"].clone()).unwrap();
        assert!(filters.hp_enabled);
        assert!((filters.hp_cutoff - 150.0).abs() < f32::EPSILON);
        assert!(filters.lp_enabled);
        assert!((filters.lp_cutoff - 7000.0).abs() < f32::EPSILON);

        let stages = obj["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 2);
        assert!(stages[0].get("Preamp").is_some());
        assert!(stages[1].get("Level").is_some());
    }

    #[test]
    fn test_migrate_preset_skips_if_already_migrated() {
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{
                "name": "Test",
                "stages": [{"Preamp": {"gain": 1.0, "bias": 0.0, "clipper_type": "ClassA"}}],
                "input_filters": {"hp_enabled": true, "hp_cutoff": 100.0, "lp_enabled": false, "lp_cutoff": 8000.0},
                "ir_name": null,
                "ir_gain": 0.1,
                "pitch_shift_semitones": 0
            }"#,
        )
        .unwrap();

        migrate_preset(&mut value);

        let stages = value["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 1);
    }

    #[test]
    fn test_migrate_preset_filter_without_cutoff() {
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{
                "name": "Test",
                "stages": [
                    {"Filter": {"filter_type": "Highpass"}},
                    {"Preamp": {"gain": 1.0, "bias": 0.0, "clipper_type": "ClassA"}}
                ],
                "ir_name": null,
                "ir_gain": 0.1,
                "pitch_shift_semitones": 0
            }"#,
        )
        .unwrap();

        migrate_preset(&mut value);

        let filters: InputFilterConfig =
            serde_json::from_value(value["input_filters"].clone()).unwrap();
        assert!(filters.hp_enabled);
        assert!((filters.hp_cutoff - 100.0).abs() < f32::EPSILON); // default cutoff
        assert!(!filters.lp_enabled);
    }
}
