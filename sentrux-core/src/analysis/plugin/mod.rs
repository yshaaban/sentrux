//! Language plugin system — runtime-loaded tree-sitter grammars.
//!
//! Plugins live in ~/.sentrux/plugins/<lang>/ and follow the Sentrux Plugin Spec:
//! - plugin.toml (manifest with metadata, capabilities, checksums)
//! - grammars/<platform>.so|.dylib (compiled tree-sitter grammar)
//! - queries/tags.scm (tree-sitter queries for structural extraction)
//!
//! Plugins are loaded at startup and registered alongside built-in languages.
//! Plugin languages take priority over built-in (allows user overrides).

pub mod embedded;
pub mod loader;
pub mod manifest;
pub mod profile;

pub use loader::{LoadedPlugin, PluginLoadError, load_all_plugins, plugins_dir};
pub use manifest::PluginManifest;
pub use profile::{LanguageProfile, LanguageSemantics, LanguageThresholds, ComplexityNodes, ProjectConfig, ResolverConfig, DEFAULT_PROFILE};

/// Silently sync embedded plugin configs to ~/.sentrux/plugins/ at startup.
/// Overwrites plugin.toml and tags.scm if the binary version is newer.
/// Preserves grammar .dylib files (expensive, platform-specific).
/// Users never need to think about plugin versions.
pub fn sync_embedded_plugins() {
    let dir = match plugins_dir() {
        Some(d) => d,
        None => return,
    };

    for &(name, toml_content, scm_content) in embedded::EMBEDDED_PLUGINS {
        let plugin_dir = dir.join(name);
        let toml_path = plugin_dir.join("plugin.toml");
        let scm_dir = plugin_dir.join("queries");
        let scm_path = scm_dir.join("tags.scm");

        // Check if config needs update (missing or different version)
        let needs_update = if toml_path.exists() {
            // Compare embedded version vs installed version
            let installed = std::fs::read_to_string(&toml_path).unwrap_or_default();
            let installed_ver = installed.lines()
                .find(|l| l.starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
                .unwrap_or("0.0.0");
            let embedded_ver = toml_content.lines()
                .find(|l| l.starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
                .unwrap_or("0.0.0");
            installed_ver != embedded_ver
        } else {
            true // Missing — needs creation
        };

        if !needs_update {
            continue;
        }

        // Create directories
        let _ = std::fs::create_dir_all(&plugin_dir);
        let _ = std::fs::create_dir_all(&scm_dir);
        let _ = std::fs::create_dir_all(plugin_dir.join("grammars"));

        // Write plugin.toml and tags.scm — preserve grammar .dylib
        let _ = std::fs::write(&toml_path, toml_content);
        if !scm_content.is_empty() {
            let _ = std::fs::write(&scm_path, scm_content);
        }
    }
}
