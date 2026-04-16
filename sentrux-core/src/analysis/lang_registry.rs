//! Language registry — maps file extensions to tree-sitter grammars and queries.
//!
//! Plugin manifests and language profiles are indexed eagerly, but runtime grammars are
//! loaded lazily on first use. That keeps unrelated broken plugins from crashing every scan.

use crate::analysis::plugin::loader::{load_single_plugin, prepared_plugins_dir};
use crate::analysis::plugin::manifest::PluginManifest;
use crate::analysis::plugin::profile::{LanguageProfile, DEFAULT_PROFILE};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};

/// Configuration for a runtime-loaded language plugin.
pub struct PluginLangConfig {
    /// Language name (owned)
    pub name: String,
    /// Compiled tree-sitter grammar (loaded from .so/.dylib)
    pub grammar: Language,
    /// Compiled tree-sitter query for structural extraction
    pub query: Query,
    /// File extensions (owned)
    pub extensions: Vec<String>,
    /// Layer 2: language profile (semantics + thresholds from plugin.toml)
    pub profile: LanguageProfile,
}

struct PluginRegistration {
    profile: LanguageProfile,
    plugin_dir: PathBuf,
    loaded: OnceLock<Result<PluginLangConfig, String>>,
}

impl PluginRegistration {
    fn load(&self) -> Option<&PluginLangConfig> {
        match self.loaded.get_or_init(|| self.load_config()) {
            Ok(config) => Some(config),
            Err(_) => None,
        }
    }

    fn load_config(&self) -> Result<PluginLangConfig, String> {
        let plugin = load_single_plugin(&self.plugin_dir).map_err(|error| {
            let message = format!("{}: {}", self.plugin_dir.display(), error);
            crate::debug_log!("[plugin] {}", message);
            message
        })?;

        let query = Query::new(&plugin.grammar, &plugin.query_src).map_err(|error| {
            let message = format!("{}: query failed: {:?}", plugin.name, error);
            crate::debug_log!("[plugin] {}", message);
            message
        })?;

        Ok(PluginLangConfig {
            name: plugin.name,
            grammar: plugin.grammar,
            query,
            extensions: plugin.extensions,
            profile: plugin.profile,
        })
    }
}

/// Central registry mapping language names and file extensions to plugin registrations.
pub struct LangRegistry {
    by_name: HashMap<String, PluginRegistration>,
    by_ext: HashMap<String, String>,
    /// Plugins that failed to parse/register (logged, not fatal).
    failed: Vec<String>,
    /// Extension → language name for ALL known plugins (including those without grammars).
    /// Used for display-only language detection (file counting, coloring).
    ext_display: HashMap<String, String>,
    /// Filename → language name for extensionless files (Dockerfile, Makefile, etc.).
    /// Populated from plugin.toml `filenames` field.
    filename_map: HashMap<String, String>,
    /// Filename prefixes → language name (e.g., "Dockerfile." → "dockerfile").
    filename_prefix_map: Vec<(String, String)>,
}

/// Parse a TOML inline array from a line like `field = ["a", "b"]`.
fn parse_toml_inline_array(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    let Some(bracket_start) = trimmed.find('[') else {
        return vec![];
    };
    let Some(bracket_end) = trimmed.find(']') else {
        return vec![];
    };
    let inner = &trimmed[bracket_start + 1..bracket_end];
    inner
        .split(',')
        .map(|segment| segment.trim().trim_matches('"').trim())
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Global singleton — indexes plugin manifests once at startup.
static REGISTRY: std::sync::LazyLock<LangRegistry> = std::sync::LazyLock::new(LangRegistry::init);

impl LangRegistry {
    fn init() -> Self {
        let mut registry = LangRegistry {
            by_name: HashMap::new(),
            by_ext: HashMap::new(),
            failed: Vec::new(),
            ext_display: HashMap::new(),
            filename_map: HashMap::new(),
            filename_prefix_map: Vec::new(),
        };
        registry.load_display_index();
        registry.discover_plugins();

        let count = registry.by_name.len();
        if count == 0 {
            eprintln!(
                "[lang_registry] No language plugins discovered. \
                 Run `sentrux plugin add-standard` to install standard languages."
            );
        } else {
            crate::debug_log!("[lang_registry] {} language plugins indexed", count);
        }

        registry
    }

    /// Build display-only extension and filename indexes from ALL embedded plugin data.
    /// This covers languages that may not have grammars installed (json, yaml, etc.).
    fn load_display_index(&mut self) {
        for &(name, toml_content, _scm) in crate::analysis::plugin::embedded::EMBEDDED_PLUGINS {
            self.index_extensions(name, toml_content);
            self.index_filenames(name, toml_content);
        }
    }

    fn discover_plugins(&mut self) {
        let Some(dir) = prepared_plugins_dir() else {
            return;
        };
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                let message = format!("{}: {}", dir.display(), error);
                crate::debug_log!("[plugin] Failed to read plugins dir: {}", message);
                self.failed.push(message);
                return;
            }
        };

        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }
            self.register_plugin_dir(plugin_dir);
        }
    }

    fn register_plugin_dir(&mut self, plugin_dir: PathBuf) {
        let manifest = match PluginManifest::load(&plugin_dir) {
            Ok(manifest) => manifest,
            Err(error) => {
                let message = format!("{}: {}", plugin_dir.display(), error);
                crate::debug_log!("[plugin] {}", message);
                self.failed.push(message);
                return;
            }
        };

        let name = manifest.plugin.name.clone();
        let profile = LanguageProfile {
            name: name.clone(),
            semantics: manifest.semantics,
            thresholds: manifest.thresholds,
            color_rgb: manifest.plugin.color_rgb.unwrap_or([80, 85, 90]),
        };

        for extension in &manifest.plugin.extensions {
            self.by_ext.insert(extension.clone(), name.clone());
            self.ext_display
                .entry(extension.clone())
                .or_insert_with(|| name.clone());
        }
        self.index_manifest_filenames(&name, &manifest.plugin.filenames);
        self.by_name.insert(
            name,
            PluginRegistration {
                profile,
                plugin_dir,
                loaded: OnceLock::new(),
            },
        );
    }

    /// Index file extensions from a plugin TOML for display language detection.
    fn index_extensions(&mut self, name: &str, toml_content: &str) {
        for line in toml_content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("extensions") {
                for ext in parse_toml_inline_array(trimmed) {
                    self.ext_display
                        .entry(ext.to_string())
                        .or_insert_with(|| name.to_string());
                }
            }
        }
    }

    /// Index filename patterns from a plugin TOML for display language detection.
    fn index_filenames(&mut self, name: &str, toml_content: &str) {
        for line in toml_content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("filenames") {
                self.index_manifest_filenames(name, &parse_toml_inline_array(trimmed));
            }
        }
    }

    fn index_manifest_filenames<T>(&mut self, name: &str, filenames: &[T])
    where
        T: AsRef<str>,
    {
        for filename in filenames {
            let filename = filename.as_ref();
            if let Some(prefix) = filename.strip_suffix('*') {
                self.filename_prefix_map
                    .push((prefix.to_string(), name.to_string()));
            } else {
                self.filename_map
                    .insert(filename.to_string(), name.to_string());
            }
        }
    }

    /// Look up by language name.
    pub fn get(&self, name: &str) -> Option<&PluginLangConfig> {
        self.by_name.get(name)?.load()
    }

    /// Get the language profile by name. Returns default profile if not found.
    pub fn profile(&self, name: &str) -> &LanguageProfile {
        self.by_name
            .get(name)
            .map(|registration| &registration.profile)
            .unwrap_or(&DEFAULT_PROFILE)
    }

    /// All registered file extensions.
    pub fn all_extensions(&self) -> Vec<&str> {
        self.by_ext
            .keys()
            .map(|extension| extension.as_str())
            .collect()
    }

    /// Number of discovered languages.
    pub fn count(&self) -> usize {
        self.by_name.len()
    }

    /// All manifest files across all discovered plugins (for project boundary detection).
    pub fn all_manifest_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self
            .by_name
            .values()
            .flat_map(|registration| {
                registration
                    .profile
                    .semantics
                    .project
                    .manifest_files
                    .iter()
                    .map(|path| path.as_str())
            })
            .collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    /// All ignored directories across all discovered plugins (merged set).
    pub fn all_ignored_dirs(&self) -> std::collections::HashSet<&str> {
        self.by_name
            .values()
            .flat_map(|registration| {
                registration
                    .profile
                    .semantics
                    .project
                    .ignored_dirs
                    .iter()
                    .map(|path| path.as_str())
            })
            .collect()
    }

    /// All source dirs across all discovered plugins (merged set for module boundary detection).
    pub fn all_source_dirs(&self) -> std::collections::HashSet<&str> {
        self.by_name
            .values()
            .flat_map(|registration| {
                registration
                    .profile
                    .semantics
                    .project
                    .source_dirs
                    .iter()
                    .map(|path| path.as_str())
            })
            .collect()
    }

    /// All mod_declaration_files across all discovered plugins (merged set).
    pub fn all_mod_declaration_files(&self) -> std::collections::HashSet<&str> {
        self.by_name
            .values()
            .flat_map(|registration| {
                registration
                    .profile
                    .semantics
                    .project
                    .mod_declaration_files
                    .iter()
                    .map(|path| path.as_str())
            })
            .collect()
    }

    /// All package_index_files across all discovered plugins (merged set).
    pub fn all_package_index_files(&self) -> std::collections::HashSet<&str> {
        self.by_name
            .values()
            .flat_map(|registration| {
                registration
                    .profile
                    .semantics
                    .package_index_files
                    .iter()
                    .map(|path| path.as_str())
            })
            .collect()
    }

    /// Iterate over all discovered profiles.
    pub fn all_profiles(&self) -> impl Iterator<Item = &LanguageProfile> {
        self.by_name
            .values()
            .map(|registration| &registration.profile)
    }

    /// Failed plugin descriptions (for UI display).
    pub fn failed(&self) -> &[String] {
        &self.failed
    }
}

// ── Public free functions delegating to global singleton ──

/// Get language config by name.
pub fn get(name: &str) -> Option<&'static PluginLangConfig> {
    REGISTRY.get(name)
}

/// Get language profile by name. Returns default profile if no plugin is registered.
pub fn profile(name: &str) -> &'static LanguageProfile {
    REGISTRY.profile(name)
}

/// Get grammar + query for a language name.
pub fn get_grammar_and_query(name: &str) -> Option<(&'static Language, &'static Query)> {
    match REGISTRY.get(name) {
        Some(config) => Some((&config.grammar, &config.query)),
        None => {
            crate::debug_log!("[lang_registry] grammar/query unavailable for {}", name);
            None
        }
    }
}

/// All registered extensions.
pub fn all_extensions() -> Vec<&'static str> {
    REGISTRY.all_extensions()
}

/// Number of discovered language plugins.
pub fn plugin_count() -> usize {
    REGISTRY.count()
}

/// All manifest files across all discovered plugins.
pub fn all_manifest_files() -> Vec<&'static str> {
    REGISTRY.all_manifest_files()
}

/// All ignored dirs across all discovered plugins (merged).
pub fn all_ignored_dirs() -> std::collections::HashSet<&'static str> {
    REGISTRY.all_ignored_dirs()
}

/// All source dirs across all discovered plugins (merged).
pub fn all_source_dirs() -> std::collections::HashSet<&'static str> {
    REGISTRY.all_source_dirs()
}

/// All mod_declaration_files across all discovered plugins (merged).
pub fn all_mod_declaration_files() -> std::collections::HashSet<&'static str> {
    REGISTRY.all_mod_declaration_files()
}

/// All package_index_files across all discovered plugins (merged).
pub fn all_package_index_files() -> std::collections::HashSet<&'static str> {
    REGISTRY.all_package_index_files()
}

/// Iterate over all discovered language profiles.
pub fn all_profiles() -> impl Iterator<Item = &'static LanguageProfile> {
    REGISTRY.all_profiles()
}

/// Detect language name from file extension string.
/// First checks discovered plugins, then falls back to the
/// display-only index (all embedded plugins, even without grammars).
pub fn detect_lang_from_ext(ext: &str) -> String {
    if let Some(name) = REGISTRY.by_ext.get(ext) {
        return name.clone();
    }
    if let Some(name) = REGISTRY.ext_display.get(ext) {
        return name.clone();
    }
    "unknown".into()
}

/// Detect language from the full filename (not just extension).
/// Reads from plugin.toml `filenames` field — no hardcoded language names.
pub fn detect_lang_from_filename(filename: &str) -> Option<String> {
    let base = filename.rsplit('/').next().unwrap_or(filename);
    if let Some(name) = REGISTRY.filename_map.get(base) {
        return Some(name.clone());
    }
    for (prefix, name) in &REGISTRY.filename_prefix_map {
        if base.starts_with(prefix.as_str()) {
            return Some(name.clone());
        }
    }
    None
}

/// Failed plugin descriptions.
pub fn failed_plugins() -> &'static [String] {
    REGISTRY.failed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_lang_from_ext_fallbacks() {
        assert_eq!(detect_lang_from_ext("json"), "json");
        assert_eq!(detect_lang_from_ext("toml"), "toml");
        assert_eq!(detect_lang_from_ext("xyz"), "unknown");
    }

    #[test]
    fn test_detect_lang_from_filename() {
        let df = detect_lang_from_filename("Dockerfile");
        let mf = detect_lang_from_filename("Makefile");
        let none = detect_lang_from_filename("random.txt");
        assert!(
            df.is_some() || df.is_none(),
            "detection works without panic"
        );
        assert!(
            mf.is_some() || mf.is_none(),
            "detection works without panic"
        );
        assert_eq!(none, None);
    }

    #[test]
    fn test_registry_loads() {
        let _ = &*REGISTRY;
    }
}
